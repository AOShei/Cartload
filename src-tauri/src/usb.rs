//! Which of the console's USB identities are attached (see `docs/device-protocol.md`): the
//! Allwinner boot ROM in FEL mode (`1f3a:efe8`, which stock firmware in normal boot shows
//! too) and the hakchi kernel's network gadget (`04e8:6863`, manufacturer `hakchi`).

use cartload_fel::{DeviceInfo, USB_PID, USB_VID};
use serde::{Deserialize, Serialize};

/// Vendor id of the hakchi kernel's RNDIS gadget (a Samsung id reused by the kernel).
pub const GADGET_VID: u16 = 0x04e8;
/// Product id of the hakchi kernel's RNDIS gadget.
pub const GADGET_PID: u16 = 0x6863;
/// Manufacturer string the gadget reports.
pub const GADGET_MANUFACTURER: &str = "hakchi";

/// A USB device, as much as enumeration tells without opening it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsbDevice {
    /// Bus the device is on (`1`).
    pub bus: String,
    /// Address on that bus.
    pub address: u8,
    /// USB vendor id.
    pub vendor_id: u16,
    /// USB product id.
    pub product_id: u16,
    /// Manufacturer string, when readable.
    pub manufacturer: Option<String>,
    /// Product string, when readable.
    pub product: Option<String>,
    /// Serial number, when readable.
    pub serial: Option<String>,
}

impl UsbDevice {
    /// Copies the enumeration data of a device.
    pub fn from_info(info: &DeviceInfo) -> Self {
        Self {
            bus: info.bus_id().to_string(),
            address: info.device_address(),
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            manufacturer: info.manufacturer_string().map(str::to_string),
            product: info.product_string().map(str::to_string),
            serial: info.serial_number().map(str::to_string),
        }
    }

    /// Identifies the device while it stays plugged in: bus and address.
    pub fn key(&self) -> String {
        format!("{}:{}", self.bus, self.address)
    }

    /// `vendor:product` in the `lsusb` notation.
    pub fn id_text(&self) -> String {
        format!("{:04x}:{:04x}", self.vendor_id, self.product_id)
    }

    /// Whether the ids are the Allwinner boot ROM's (FEL mode, or stock firmware in normal
    /// boot, which uses the same ids).
    pub fn has_fel_id(&self) -> bool {
        self.vendor_id == USB_VID && self.product_id == USB_PID
    }

    /// Whether this is the hakchi kernel's network gadget. The ids belong to Samsung; the
    /// manufacturer string tells a console from a phone in tethering mode. A missing string
    /// (not readable) is accepted.
    pub fn is_gadget(&self) -> bool {
        self.vendor_id == GADGET_VID
            && self.product_id == GADGET_PID
            && self
                .manufacturer
                .as_deref()
                .is_none_or(|m| m.trim().eq_ignore_ascii_case(GADGET_MANUFACTURER))
    }

    /// Whether the product string looks like stock firmware in normal boot (`CLV-S-SNPY`).
    pub fn looks_like_stock_firmware(&self) -> bool {
        self.product
            .as_deref()
            .is_some_and(|p| p.to_ascii_uppercase().contains("CLV-"))
    }
}

/// The console-related devices found on the bus.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsbScan {
    /// Devices with the FEL ids.
    pub fel: Vec<UsbDevice>,
    /// hakchi network gadgets.
    pub gadgets: Vec<UsbDevice>,
}

impl UsbScan {
    /// Sorts devices into the two identities, dropping everything else.
    pub fn classify(devices: impl IntoIterator<Item = UsbDevice>) -> Self {
        let mut scan = Self::default();
        for device in devices {
            if device.has_fel_id() {
                scan.fel.push(device);
            } else if device.is_gadget() {
                scan.gadgets.push(device);
            }
        }
        scan
    }

    /// Whether a hakchi kernel (installed or membooted) is enumerated.
    pub fn gadget_present(&self) -> bool {
        !self.gadgets.is_empty()
    }
}

/// Enumerates the bus.
pub async fn scan() -> Result<UsbScan, nusb::Error> {
    let devices = nusb::list_devices().await?;
    Ok(UsbScan::classify(
        devices.map(|info| UsbDevice::from_info(&info)),
    ))
}

/// Finds the enumeration handle of a FEL device by its [`UsbDevice::key`], for opening it.
pub async fn find_fel_info(key: &str) -> Result<Option<DeviceInfo>, cartload_fel::Error> {
    Ok(cartload_fel::find_devices()
        .await?
        .into_iter()
        .find(|info| UsbDevice::from_info(info).key() == key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(vendor_id: u16, product_id: u16, manufacturer: Option<&str>) -> UsbDevice {
        UsbDevice {
            bus: "1".into(),
            address: 7,
            vendor_id,
            product_id,
            manufacturer: manufacturer.map(str::to_string),
            product: None,
            serial: None,
        }
    }

    #[test]
    fn classifies_the_two_identities() {
        let scan = UsbScan::classify([
            device(0x1f3a, 0xefe8, None),
            device(0x04e8, 0x6863, Some("hakchi")),
            device(0x04e8, 0x6863, Some("SAMSUNG")),
            device(0x04e8, 0x6863, None),
            device(0x046d, 0xc52b, Some("Logitech")),
        ]);
        assert_eq!(scan.fel.len(), 1);
        assert_eq!(
            scan.gadgets.len(),
            2,
            "hakchi and unreadable, not the phone"
        );
        assert!(scan.gadget_present());
        assert_eq!(scan.fel[0].id_text(), "1f3a:efe8");
        assert_eq!(scan.fel[0].key(), "1:7");
    }

    #[test]
    fn stock_firmware_product_string() {
        let mut stock = device(0x1f3a, 0xefe8, Some("Nintendo"));
        stock.product = Some("CLV-S-SNPY".into());
        assert!(stock.looks_like_stock_firmware());
        assert!(!device(0x1f3a, 0xefe8, None).looks_like_stock_firmware());
    }
}
