//! Placeholder images for folders and games without art. The console shows a 228×204 cover
//! and a 40×40 thumbnail per entry (hakchi2 CE `NesMenuFolder.Save`, `NesApplication.SetImage`).
//! Real cover art comes with task 07; these keep the menu complete until then.

use std::io::Cursor;

use crate::{Error, Result, menu::FolderImage, rom::System};

/// Width of a cover image.
pub const COVER_WIDTH: u32 = 228;
/// Height of a cover image.
pub const COVER_HEIGHT: u32 = 204;
/// Side of the thumbnail (`_small.png`).
pub const THUMB_SIZE: u32 = 40;

/// An RGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// Writes an RGB image: `pixel(x, y)` gives each colour.
pub fn render(width: u32, height: u32, pixel: impl Fn(u32, u32) -> Rgb) -> Result<Vec<u8>> {
    let mut data = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            let Rgb(r, g, b) = pixel(x, y);
            data.extend_from_slice(&[r, g, b]);
        }
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut out), width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| Error::Invalid(format!("PNG header: {e}")))?;
        writer
            .write_image_data(&data)
            .map_err(|e| Error::Invalid(format!("PNG data: {e}")))?;
    }
    Ok(out)
}

/// A flat tile with a border and, for folders, a tab along the top edge.
fn tile(width: u32, height: u32, fill: Rgb, border: Rgb, tab: bool) -> Result<Vec<u8>> {
    let border_px = (width / 38).max(1);
    let tab_height = height / 8;
    let tab_width = width * 2 / 5;
    render(width, height, |x, y| {
        let on_border =
            x < border_px || y < border_px || x >= width - border_px || y >= height - border_px;
        if on_border {
            return border;
        }
        if tab && y < tab_height + border_px && x < tab_width {
            return border;
        }
        fill
    })
}

fn folder_colours(image: FolderImage) -> (Rgb, Rgb) {
    match image {
        FolderImage::Folder => (Rgb(0x3b, 0x5b, 0x8c), Rgb(0xd7, 0xe0, 0xf0)),
        FolderImage::Back => (Rgb(0x5a, 0x5a, 0x5a), Rgb(0xe6, 0xe6, 0xe6)),
        FolderImage::Home => (Rgb(0x2f, 0x6b, 0x4f), Rgb(0xd5, 0xee, 0xdf)),
        FolderImage::Number => (Rgb(0x7a, 0x4e, 0x8a), Rgb(0xec, 0xdc, 0xf2)),
        FolderImage::Letter(_) => (Rgb(0x8a, 0x5a, 0x2b), Rgb(0xf2, 0xe4, 0xd2)),
    }
}

/// The 228×204 icon of a folder.
pub fn folder_icon(image: FolderImage) -> Result<Vec<u8>> {
    let (fill, border) = folder_colours(image);
    tile(COVER_WIDTH, COVER_HEIGHT, fill, border, true)
}

/// The 40×40 thumbnail of a folder.
pub fn folder_thumbnail(image: FolderImage) -> Result<Vec<u8>> {
    let (fill, border) = folder_colours(image);
    tile(THUMB_SIZE, THUMB_SIZE, fill, border, true)
}

fn system_colours(system: System) -> (Rgb, Rgb) {
    match system {
        System::Snes => (Rgb(0x4a, 0x4a, 0x5c), Rgb(0xb0, 0xb0, 0xc8)),
        System::Nes => (Rgb(0x6e, 0x6e, 0x6e), Rgb(0xc9, 0x3a, 0x3a)),
        System::MegaDrive => (Rgb(0x20, 0x20, 0x20), Rgb(0xaa, 0xaa, 0xaa)),
    }
}

/// The 228×204 placeholder cover of a game without art.
pub fn cover_placeholder(system: System) -> Result<Vec<u8>> {
    let (fill, border) = system_colours(system);
    tile(COVER_WIDTH, COVER_HEIGHT, fill, border, false)
}

/// The 40×40 placeholder thumbnail of a game without art.
pub fn thumbnail_placeholder(system: System) -> Result<Vec<u8>> {
    let (fill, border) = system_colours(system);
    tile(THUMB_SIZE, THUMB_SIZE, fill, border, false)
}

/// Reads the dimensions of a PNG file's header.
pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let reader = decoder.read_info().ok()?;
    let info = reader.info();
    Some((info.width, info.height))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn images_have_the_console_sizes() {
        let icon = folder_icon(FolderImage::Folder).unwrap();
        assert_eq!(png_dimensions(&icon), Some((228, 204)));
        let thumb = folder_thumbnail(FolderImage::Letter('A')).unwrap();
        assert_eq!(png_dimensions(&thumb), Some((40, 40)));
        assert_eq!(
            png_dimensions(&cover_placeholder(System::Nes).unwrap()),
            Some((228, 204))
        );
        assert_eq!(
            png_dimensions(&thumbnail_placeholder(System::Snes).unwrap()),
            Some((40, 40))
        );
        assert_eq!(png_dimensions(b"not a png"), None);
    }

    #[test]
    fn images_are_deterministic() {
        assert_eq!(
            folder_icon(FolderImage::Back).unwrap(),
            folder_icon(FolderImage::Back).unwrap()
        );
        assert_ne!(
            folder_icon(FolderImage::Back).unwrap(),
            folder_icon(FolderImage::Folder).unwrap()
        );
    }
}
