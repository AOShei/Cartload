//! USB pipe to a FEL device.
//!
//! [`Transport`] is the raw bulk pipe the protocol layer in [`crate::Device`] talks through.
//! [`UsbTransport`] implements it over nusb; tests implement it with a scripted mock.

use std::{future::Future, time::Duration};

use nusb::{
    DeviceInfo, Endpoint, Interface,
    descriptors::TransferType,
    transfer::{Buffer, Bulk, Completion, Direction, EndpointDirection, In, Out},
};
use tracing::{debug, trace, warn};

use crate::{Error, Result, USB_PID, USB_VID};

/// Default per-transfer timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

/// Bulk endpoint addresses the boot ROM is known to use (IN, OUT).
pub const EXPECTED_ENDPOINTS: (u8, u8) = (0x82, 0x01);

/// A raw bulk pipe to a FEL device: writes go to the OUT endpoint, reads come from IN.
pub trait Transport: Send {
    /// Write all of `data` to the device.
    fn write(&mut self, data: &[u8]) -> impl Future<Output = Result<()>> + Send;

    /// Read exactly `len` bytes from the device.
    fn read(&mut self, len: usize) -> impl Future<Output = Result<Vec<u8>>> + Send;
}

/// All connected USB devices with the FEL vendor and product id.
///
/// A console in normal boot with stock firmware shows up here too; only
/// [`Device::verify_device`](crate::Device::verify_device) tells them apart.
pub async fn find_devices() -> Result<Vec<DeviceInfo>> {
    Ok(nusb::list_devices()
        .await?
        .filter(|d| d.vendor_id() == USB_VID && d.product_id() == USB_PID)
        .collect())
}

/// Poll until a FEL device appears or `timeout` passes.
pub async fn wait_for_device(timeout: Duration) -> Result<DeviceInfo> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(info) = find_devices().await?.into_iter().next() {
            return Ok(info);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(Error::DeviceNotFound);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// nusb-backed transport over the boot ROM's bulk endpoint pair.
pub struct UsbTransport {
    _device: nusb::Device,
    _interface: Interface,
    ep_in: Endpoint<Bulk, In>,
    ep_out: Endpoint<Bulk, Out>,
    timeout: Duration,
}

fn open_error(err: nusb::Error) -> Error {
    if err.kind() == nusb::ErrorKind::PermissionDenied {
        Error::PermissionDenied(err)
    } else {
        Error::Usb(err)
    }
}

impl UsbTransport {
    /// Open the device, select configuration 1 if needed, claim interface 0 and locate the
    /// bulk endpoint pair.
    pub async fn open(info: &DeviceInfo) -> Result<Self> {
        debug!(
            bus = info.bus_id(),
            address = info.device_address(),
            speed = ?info.speed(),
            product = info.product_string().unwrap_or(""),
            "opening FEL device"
        );
        let device = info.open().await.map_err(open_error)?;
        if device.active_configuration().is_err() {
            debug!("device is unconfigured; selecting configuration 1");
            device.set_configuration(1).await?;
        }
        let interface = device
            .detach_and_claim_interface(0)
            .await
            .map_err(open_error)?;

        let mut in_addr = None;
        let mut out_addr = None;
        if let Some(descriptor) = interface.descriptor() {
            for ep in descriptor.endpoints() {
                if ep.transfer_type() != TransferType::Bulk {
                    continue;
                }
                match ep.direction() {
                    Direction::In => in_addr.get_or_insert(ep.address()),
                    Direction::Out => out_addr.get_or_insert(ep.address()),
                };
            }
        }
        let (Some(in_addr), Some(out_addr)) = (in_addr, out_addr) else {
            return Err(Error::NoBulkEndpoints);
        };
        if (in_addr, out_addr) != EXPECTED_ENDPOINTS {
            warn!(
                "bulk endpoints are IN {in_addr:#04x} / OUT {out_addr:#04x}, expected \
                 {:#04x} / {:#04x}",
                EXPECTED_ENDPOINTS.0, EXPECTED_ENDPOINTS.1
            );
        } else {
            debug!("bulk endpoints: IN {in_addr:#04x}, OUT {out_addr:#04x}");
        }
        let ep_in = interface.endpoint::<Bulk, In>(in_addr)?;
        let ep_out = interface.endpoint::<Bulk, Out>(out_addr)?;
        Ok(Self {
            _device: device,
            _interface: interface,
            ep_in,
            ep_out,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    /// Per-transfer timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Change the per-transfer timeout.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Endpoint addresses in use (IN, OUT).
    pub fn endpoints(&self) -> (u8, u8) {
        (
            self.ep_in.endpoint_address(),
            self.ep_out.endpoint_address(),
        )
    }
}

/// Wait for the single pending transfer on `ep`, cancelling it on timeout.
///
/// After a timeout the pipe is in an unknown state; callers should drop the device.
async fn complete<Dir: EndpointDirection>(
    ep: &mut Endpoint<Bulk, Dir>,
    timeout: Duration,
    op: &'static str,
) -> Result<Completion> {
    match tokio::time::timeout(timeout, ep.next_complete()).await {
        Ok(completion) => Ok(completion),
        Err(_) => {
            ep.cancel_all();
            if tokio::time::timeout(Duration::from_secs(1), ep.next_complete())
                .await
                .is_err()
            {
                warn!("cancelled {op} transfer has not returned");
            }
            Err(Error::Timeout { op, timeout })
        }
    }
}

impl Transport for UsbTransport {
    async fn write(&mut self, data: &[u8]) -> Result<()> {
        const OP: &str = "bulk write";
        trace!(len = data.len(), "-> usb");
        self.ep_out.submit(Buffer::from(data.to_vec()));
        let completion = complete(&mut self.ep_out, self.timeout, OP).await?;
        completion.status?;
        if completion.actual_len != data.len() {
            return Err(Error::ShortTransfer {
                op: OP,
                expected: data.len(),
                actual: completion.actual_len,
            });
        }
        Ok(())
    }

    async fn read(&mut self, len: usize) -> Result<Vec<u8>> {
        const OP: &str = "bulk read";
        let packet = self.ep_in.max_packet_size().max(1);
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            let remaining = len - out.len();
            // IN transfers must request a whole number of packets; a short packet ends them.
            let requested = remaining.div_ceil(packet) * packet;
            self.ep_in.submit(Buffer::new(requested));
            let completion = complete(&mut self.ep_in, self.timeout, OP).await?;
            let status = completion.status;
            let chunk = completion.buffer.into_vec();
            status?;
            if chunk.is_empty() {
                return Err(Error::ShortTransfer {
                    op: OP,
                    expected: len,
                    actual: out.len(),
                });
            }
            if chunk.len() > remaining {
                warn!(
                    "device sent {} bytes, {} more than requested",
                    chunk.len(),
                    chunk.len() - remaining
                );
            }
            let take = chunk.len().min(remaining);
            out.extend_from_slice(&chunk[..take]);
        }
        trace!(len, "<- usb");
        Ok(out)
    }
}
