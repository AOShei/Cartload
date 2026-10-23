//! Cover images: the sizes the console shows, the resize rules of hakchi2 CE
//! (`NesMenuElementBase.SetImage`, `Shared.ResizeImage`) and the placeholder tiles used until
//! real art is fetched.
//!
//! The console shows a cover of at most 228×204 and a 40×40 thumbnail (`_small.png`) per entry.
//! hakchi2 CE fits the cover into 228×204 keeping proportions and upscaling small images, fits
//! the thumbnail into 40×40 without upscaling, rounds odd dimensions up to even, and by default
//! writes 8-bit palette PNGs (`CompressCover`). Those rules are reproduced here with the
//! `image` crate (Catmull-Rom, the bicubic filter GDI+ `HighQualityBicubic` approximates) and a
//! NeuQuant palette.

use std::{collections::BTreeMap, io::Cursor};

use image::{DynamicImage, RgbaImage, imageops::FilterType};

use crate::{Error, Result, menu::FolderImage, rom::System};

/// Width of a cover image.
pub const COVER_WIDTH: u32 = 228;
/// Height of a cover image.
pub const COVER_HEIGHT: u32 = 204;
/// Side of the thumbnail (`_small.png`).
pub const THUMB_SIZE: u32 = 40;

/// Options for [`make_cover`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoverOptions {
    /// Pad the thumbnail to 40 pixels high with the image centred (hakchi2 CE's
    /// `CenterThumbnail`, off by default).
    pub center_thumbnail: bool,
    /// Write 8-bit palette PNGs (hakchi2 CE's `CompressCover`, on by default): about a third
    /// of the size of true-colour PNGs on the console's small flash.
    pub quantize: bool,
}

impl Default for CoverOptions {
    fn default() -> Self {
        Self {
            center_thumbnail: false,
            quantize: true,
        }
    }
}

/// The two PNGs the console needs for one game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverSet {
    /// `CLV-U-XXXXX.png`, at most 228×204.
    pub cover: Vec<u8>,
    /// `CLV-U-XXXXX_small.png`, at most 40×40.
    pub thumbnail: Vec<u8>,
    /// Dimensions of the cover.
    pub cover_size: (u32, u32),
    /// Dimensions of the thumbnail.
    pub thumbnail_size: (u32, u32),
    /// Dimensions of the source image.
    pub source_size: (u32, u32),
}

/// Decodes a PNG, JPEG, GIF or BMP image.
pub fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    image::load_from_memory(bytes).map_err(|e| Error::Image(e.to_string()))
}

/// The size an image gets when fitted into `target_width`×`target_height` keeping its
/// proportions: hakchi2 CE's `Shared.ResizeImage` with `keepProportions`. Images that already
/// fit are left alone unless `upscale` is set; odd results are rounded up to even.
pub fn fit_size(
    (width, height): (u32, u32),
    (target_width, target_height): (u32, u32),
    upscale: bool,
) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (0, 0);
    }
    if !upscale && width <= target_width && height <= target_height {
        return (width, height);
    }
    let even = |value: f64| {
        let rounded = value.round().max(1.0);
        let rounded = if rounded as u64 % 2 == 1 {
            rounded + 1.0
        } else {
            rounded
        };
        rounded as u32
    };
    if f64::from(width) / f64::from(height) > f64::from(target_width) / f64::from(target_height) {
        (
            target_width,
            even(f64::from(target_width) * f64::from(height) / f64::from(width)),
        )
    } else {
        (
            even(f64::from(target_height) * f64::from(width) / f64::from(height)),
            target_height,
        )
    }
}

/// Fits an image into a box as [`fit_size`] says. With `expand_height` the canvas is
/// `target.1` high and the image is centred vertically on transparent pixels (hakchi2 CE's
/// `expandHeight`, used for centred thumbnails).
pub fn fit(
    image: &DynamicImage,
    target: (u32, u32),
    upscale: bool,
    expand_height: bool,
) -> RgbaImage {
    let source = (image.width(), image.height());
    let (width, height) = fit_size(source, target, upscale);
    let resized = if (width, height) == source {
        image.to_rgba8()
    } else {
        image
            .resize_exact(width, height, FilterType::CatmullRom)
            .to_rgba8()
    };
    if expand_height && height < target.1 {
        let mut canvas = RgbaImage::new(width, target.1);
        let offset = i64::from((target.1 - height) / 2);
        image::imageops::overlay(&mut canvas, &resized, 0, offset);
        return canvas;
    }
    resized
}

/// Builds the cover and the thumbnail of a game from any supported image, as hakchi2 CE's
/// `SetImage` does.
pub fn make_cover(bytes: &[u8], options: &CoverOptions) -> Result<CoverSet> {
    let image = decode(bytes)?;
    let source_size = (image.width(), image.height());
    if source_size.0 == 0 || source_size.1 == 0 {
        return Err(Error::Image("the image is empty".into()));
    }
    let cover = fit(&image, (COVER_WIDTH, COVER_HEIGHT), true, false);
    let thumbnail = fit(
        &image,
        (THUMB_SIZE, THUMB_SIZE),
        false,
        options.center_thumbnail,
    );
    Ok(CoverSet {
        cover_size: cover.dimensions(),
        thumbnail_size: thumbnail.dimensions(),
        cover: encode_png(&cover, options.quantize)?,
        thumbnail: encode_png(&thumbnail, options.quantize)?,
        source_size,
    })
}

/// Encodes an image as PNG: an 8-bit palette (exact when the image has at most 256 colours,
/// NeuQuant otherwise) when `quantize` is set, else RGB or RGBA 8-bit.
pub fn encode_png(image: &RgbaImage, quantize: bool) -> Result<Vec<u8>> {
    let (width, height) = image.dimensions();
    let pixels = image.as_raw();
    let opaque = rgba(pixels).iter().all(|p| p[3] == 0xFF);
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(Cursor::new(&mut out), width, height);
    encoder.set_depth(png::BitDepth::Eight);
    let data: Vec<u8> = if quantize {
        let (palette, indices) = palette(pixels);
        encoder.set_color(png::ColorType::Indexed);
        let rgb: Vec<u8> = rgba(&palette)
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        encoder.set_palette(rgb);
        if !opaque {
            let alpha: Vec<u8> = rgba(&palette).iter().map(|p| p[3]).collect();
            encoder.set_trns(alpha);
        }
        indices
    } else if opaque {
        encoder.set_color(png::ColorType::Rgb);
        rgba(pixels)
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect()
    } else {
        encoder.set_color(png::ColorType::Rgba);
        pixels.to_vec()
    };
    let mut writer = encoder
        .write_header()
        .map_err(|e| Error::Image(format!("PNG header: {e}")))?;
    writer
        .write_image_data(&data)
        .map_err(|e| Error::Image(format!("PNG data: {e}")))?;
    drop(writer);
    Ok(out)
}

/// The pixels of an RGBA buffer as 4-byte groups (a trailing partial pixel is ignored).
fn rgba(pixels: &[u8]) -> &[[u8; 4]] {
    pixels.as_chunks::<4>().0
}

/// A palette of at most 256 RGBA entries and one index per pixel.
fn palette(pixels: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut exact: BTreeMap<[u8; 4], u8> = BTreeMap::new();
    for key in rgba(pixels).iter().copied() {
        if !exact.contains_key(&key) {
            if exact.len() == 256 {
                exact.clear();
                break;
            }
            let next = exact.len() as u8;
            exact.insert(key, next);
        }
    }
    if !exact.is_empty() || pixels.is_empty() {
        let mut palette = vec![[0u8; 4]; exact.len().max(1)];
        for (colour, index) in &exact {
            palette[usize::from(*index)] = *colour;
        }
        let indices = rgba(pixels).iter().map(|p| exact[p]).collect();
        return (palette.concat(), indices);
    }
    let quantizer = color_quant::NeuQuant::new(1, 256, pixels);
    let palette = quantizer.color_map_rgba();
    let indices = rgba(pixels)
        .iter()
        .map(|p| quantizer.index_of(p) as u8)
        .collect();
    (palette, indices)
}

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
            .map_err(|e| Error::Image(format!("PNG header: {e}")))?;
        writer
            .write_image_data(&data)
            .map_err(|e| Error::Image(format!("PNG data: {e}")))?;
    }
    Ok(out)
}

/// A flat tile with a border and, for folders, a tab along the top edge (placeholders).
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

/* Folder tiles are pixel art: a 19×17 design drawn at 12× for the 228×204 icon and at 2×,
centred, for the 40×40 thumbnail. Whole-number scaling keeps every edge a clean step, like
the console's own menu art, instead of the ragged diagonals a resampled shape gets. */

/// Width of a folder design, in design pixels (228 / 12).
const DESIGN_WIDTH: usize = 19;
/// Height of a folder design, in design pixels (204 / 12).
const DESIGN_HEIGHT: usize = 17;
/// Design pixels to image pixels for the icon.
const ICON_SCALE: u32 = 12;
/// Design pixels to image pixels for the thumbnail.
const THUMB_SCALE: u32 = 2;

/// The four colours of a folder tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Palette {
    /// The tile behind the drawing.
    background: Rgb,
    /// Dark outline and shadow.
    outline: Rgb,
    /// The drawing's main colour.
    fill: Rgb,
    /// Highlights.
    light: Rgb,
}

fn folder_palette(image: &FolderImage) -> Palette {
    let (background, outline, fill, light) = match image {
        FolderImage::Folder | FolderImage::Custom(_) => (
            Rgb(0x2c, 0x3e, 0x66),
            Rgb(0x12, 0x1a, 0x30),
            Rgb(0xe8, 0xb9, 0x4a),
            Rgb(0xf8, 0xe0, 0x9a),
        ),
        FolderImage::Back => (
            Rgb(0x3a, 0x3a, 0x42),
            Rgb(0x16, 0x16, 0x1c),
            Rgb(0xd8, 0xd8, 0xe0),
            Rgb(0xff, 0xff, 0xff),
        ),
        FolderImage::Home => (
            Rgb(0x25, 0x5a, 0x44),
            Rgb(0x0e, 0x26, 0x1c),
            Rgb(0xe6, 0xe2, 0xcc),
            Rgb(0xff, 0xfb, 0xe8),
        ),
        FolderImage::Number => (
            Rgb(0x56, 0x34, 0x6e),
            Rgb(0x22, 0x12, 0x2e),
            Rgb(0xf0, 0xe0, 0xf8),
            Rgb(0xff, 0xff, 0xff),
        ),
        FolderImage::Letter(_) => (
            Rgb(0x7a, 0x3e, 0x2a),
            Rgb(0x30, 0x14, 0x0c),
            Rgb(0xf6, 0xe4, 0xc8),
            Rgb(0xff, 0xff, 0xff),
        ),
    };
    Palette {
        background,
        outline,
        fill,
        light,
    }
}

/// A classic folder: tab top left, body with a lighter band under the tab.
const FOLDER_DESIGN: [&str; DESIGN_HEIGHT] = [
    "...................",
    "...................",
    "..#######..........",
    ".#ooooooo#.........",
    ".#ooooooo#########.",
    ".#+++++++++++++++#.",
    ".#ooooooooooooooo#.",
    ".#ooooooooooooooo#.",
    ".#ooooooooooooooo#.",
    ".#ooooooooooooooo#.",
    ".#ooooooooooooooo#.",
    ".#ooooooooooooooo#.",
    ".#ooooooooooooooo#.",
    ".#ooooooooooooooo#.",
    ".#################.",
    "...................",
    "...................",
];

/// A chunky arrow pointing left: "back".
const BACK_DESIGN: [&str; DESIGN_HEIGHT] = [
    "...................",
    "...................",
    "...................",
    ".......##..........",
    "......#+#..........",
    ".....#+o#..........",
    "....#+oo##########.",
    "...#+ooooooooooooo#",
    "..#+oooooooooooooo#",
    "...#oooooooooooooo#",
    "....#ooo##########.",
    ".....#oo#..........",
    "......#o#..........",
    ".......##..........",
    "...................",
    "...................",
    "...................",
];

/// A house with a door: "home".
const HOME_DESIGN: [&str; DESIGN_HEIGHT] = [
    "...................",
    "...................",
    "........##.........",
    ".......#++#........",
    "......#+oo+#.......",
    ".....#+oooo+#......",
    "....#+oooooo+#.....",
    "...#+oooooooo+#....",
    "..####oooooo####...",
    ".....#oooooo#......",
    ".....#oo##oo#......",
    ".....#oo#.#oo#.....",
    ".....#oo#.#oo#.....",
    ".....#oo#.#oo#.....",
    ".....########......",
    "...................",
    "...................",
];

/// A 5×7 pixel font for the letter and number folders: A to Z, `#` and `?` for anything else.
fn font_glyph(c: char) -> [&'static str; 7] {
    match c.to_ascii_uppercase() {
        'A' => [
            ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ],
        'B' => [
            "####.", "#...#", "#...#", "####.", "#...#", "#...#", "####.",
        ],
        'C' => [
            ".####", "#....", "#....", "#....", "#....", "#....", ".####",
        ],
        'D' => [
            "####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####.",
        ],
        'E' => [
            "#####", "#....", "#....", "####.", "#....", "#....", "#####",
        ],
        'F' => [
            "#####", "#....", "#....", "####.", "#....", "#....", "#....",
        ],
        'G' => [
            ".####", "#....", "#....", "#.###", "#...#", "#...#", ".####",
        ],
        'H' => [
            "#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ],
        'I' => [
            "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####",
        ],
        'J' => [
            "..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##..",
        ],
        'K' => [
            "#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#",
        ],
        'L' => [
            "#....", "#....", "#....", "#....", "#....", "#....", "#####",
        ],
        'M' => [
            "#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#",
        ],
        'N' => [
            "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#",
        ],
        'O' => [
            ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
        ],
        'P' => [
            "####.", "#...#", "#...#", "####.", "#....", "#....", "#....",
        ],
        'Q' => [
            ".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#",
        ],
        'R' => [
            "####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#",
        ],
        'S' => [
            ".####", "#....", "#....", ".###.", "....#", "....#", "####.",
        ],
        'T' => [
            "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#..",
        ],
        'U' => [
            "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
        ],
        'V' => [
            "#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#..",
        ],
        'W' => [
            "#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#",
        ],
        'X' => [
            "#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#",
        ],
        'Y' => [
            "#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#..",
        ],
        'Z' => [
            "#####", "....#", "...#.", "..#..", ".#...", "#....", "#####",
        ],
        '#' => [
            ".#.#.", ".#.#.", "#####", ".#.#.", "#####", ".#.#.", ".#.#.",
        ],
        _ => [
            ".###.", "#...#", "....#", "..##.", "..#..", ".....", "..#..",
        ],
    }
}

/// A design pixel: which palette entry, or the background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ink {
    Background,
    Outline,
    Fill,
    Light,
}

fn ink(c: u8) -> Ink {
    match c {
        b'#' => Ink::Outline,
        b'o' => Ink::Fill,
        b'+' => Ink::Light,
        _ => Ink::Background,
    }
}

/// The 19×17 design of a folder image, as inks.
fn folder_design(image: &FolderImage) -> Vec<[Ink; DESIGN_WIDTH]> {
    let from_rows = |rows: [&str; DESIGN_HEIGHT]| {
        rows.iter()
            .map(|row| {
                let mut out = [Ink::Background; DESIGN_WIDTH];
                for (i, c) in row.bytes().enumerate().take(DESIGN_WIDTH) {
                    out[i] = ink(c);
                }
                out
            })
            .collect::<Vec<_>>()
    };
    match image {
        FolderImage::Folder | FolderImage::Custom(_) => from_rows(FOLDER_DESIGN),
        FolderImage::Back => from_rows(BACK_DESIGN),
        FolderImage::Home => from_rows(HOME_DESIGN),
        FolderImage::Number => letter_design('#'),
        FolderImage::Letter(c) => letter_design(*c),
    }
}

/// A font glyph at 2× (10×14), centred, in the fill colour with a one-pixel outline shadow
/// to the lower right.
fn letter_design(c: char) -> Vec<[Ink; DESIGN_WIDTH]> {
    let glyph = font_glyph(c);
    let (x0, y0) = (4usize, 1usize);
    let mut lit = vec![[false; DESIGN_WIDTH]; DESIGN_HEIGHT];
    for (gy, row) in glyph.iter().enumerate() {
        for (gx, cell) in row.bytes().enumerate() {
            if cell == b'#' {
                for dy in 0..2 {
                    for dx in 0..2 {
                        lit[y0 + gy * 2 + dy][x0 + gx * 2 + dx] = true;
                    }
                }
            }
        }
    }
    let mut design = vec![[Ink::Background; DESIGN_WIDTH]; DESIGN_HEIGHT];
    for y in 0..DESIGN_HEIGHT {
        for x in 0..DESIGN_WIDTH {
            if lit[y][x] {
                design[y][x] = Ink::Fill;
            } else if x > 0 && y > 0 && lit[y - 1][x - 1] {
                design[y][x] = Ink::Outline;
            }
        }
    }
    design
}

/// Draws a design scaled by `scale`, offset by (`dx`, `dy`), on a `width`×`height` tile.
fn pixel_tile(
    image: &FolderImage,
    width: u32,
    height: u32,
    scale: u32,
    dx: u32,
    dy: u32,
) -> Result<Vec<u8>> {
    let palette = folder_palette(image);
    let design = folder_design(image);
    render(width, height, |x, y| {
        let (Some(cx), Some(cy)) = (x.checked_sub(dx), y.checked_sub(dy)) else {
            return palette.background;
        };
        let (col, row) = ((cx / scale) as usize, (cy / scale) as usize);
        let ink = design
            .get(row)
            .and_then(|r| r.get(col))
            .copied()
            .unwrap_or(Ink::Background);
        match ink {
            Ink::Background => palette.background,
            Ink::Outline => palette.outline,
            Ink::Fill => palette.fill,
            Ink::Light => palette.light,
        }
    })
}

/// The 228×204 icon of a folder.
pub fn folder_icon(image: &FolderImage) -> Result<Vec<u8>> {
    pixel_tile(image, COVER_WIDTH, COVER_HEIGHT, ICON_SCALE, 0, 0)
}

/// The 40×40 thumbnail of a folder: the design at 2× (38×34), centred.
pub fn folder_thumbnail(image: &FolderImage) -> Result<Vec<u8>> {
    let dx = (THUMB_SIZE - DESIGN_WIDTH as u32 * THUMB_SCALE) / 2;
    let dy = (THUMB_SIZE - DESIGN_HEIGHT as u32 * THUMB_SCALE) / 2;
    pixel_tile(image, THUMB_SIZE, THUMB_SIZE, THUMB_SCALE, dx, dy)
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
pub(crate) mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// A smooth gradient with a diagonal, as JPEG or PNG; too many colours for an exact palette.
    pub(crate) fn gradient(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
        let image = RgbaImage::from_fn(width, height, |x, y| {
            let r = (x * 255 / width.max(1)) as u8;
            let g = (y * 255 / height.max(1)) as u8;
            let b = if x == y { 255 } else { ((x + y) % 251) as u8 };
            image::Rgba([r, g, b, 255])
        });
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut out, format)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn images_have_the_console_sizes() {
        let icon = folder_icon(&FolderImage::Folder).unwrap();
        assert_eq!(png_dimensions(&icon), Some((228, 204)));
        let thumb = folder_thumbnail(&FolderImage::Letter('A')).unwrap();
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
            folder_icon(&FolderImage::Back).unwrap(),
            folder_icon(&FolderImage::Back).unwrap()
        );
        assert_ne!(
            folder_icon(&FolderImage::Back).unwrap(),
            folder_icon(&FolderImage::Folder).unwrap()
        );
    }

    #[test]
    fn fit_size_follows_hakchi2_ce() {
        // A 2100×1533 SNES box (TheGamesDB original of game 1255) is wider than 228:204.
        assert_eq!(fit_size((2100, 1533), (228, 204), true), (228, 166));
        // A portrait NES box is limited by the height; odd widths round up to even.
        assert_eq!(fit_size((1000, 1400), (228, 204), true), (146, 204));
        assert_eq!(fit_size((1529, 2156), (228, 204), true), (146, 204));
        // Small images are upscaled for the cover but kept for the thumbnail.
        assert_eq!(fit_size((100, 100), (228, 204), true), (204, 204));
        assert_eq!(fit_size((30, 20), (40, 40), false), (30, 20));
        assert_eq!(fit_size((300, 200), (40, 40), false), (40, 28));
        assert_eq!(fit_size((0, 10), (40, 40), false), (0, 0));
    }

    #[test]
    fn make_cover_produces_both_sizes_from_a_jpeg() {
        let jpeg = gradient(600, 400, image::ImageFormat::Jpeg);
        let set = make_cover(&jpeg, &CoverOptions::default()).unwrap();
        assert_eq!(set.source_size, (600, 400));
        assert_eq!(set.cover_size, (228, 152));
        assert_eq!(set.thumbnail_size, (40, 28));
        assert_eq!(png_dimensions(&set.cover), Some((228, 152)));
        assert_eq!(png_dimensions(&set.thumbnail), Some((40, 28)));
        // Palette PNGs: the cover decodes back to the same size and is far smaller than RGB.
        let plain = make_cover(
            &jpeg,
            &CoverOptions {
                quantize: false,
                ..CoverOptions::default()
            },
        )
        .unwrap();
        assert!(set.cover.len() < plain.cover.len());
        let decoded = decode(&set.cover).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (228, 152));
    }

    #[test]
    fn centred_thumbnails_are_padded_to_the_full_height() {
        let png = gradient(300, 100, image::ImageFormat::Png);
        let set = make_cover(
            &png,
            &CoverOptions {
                center_thumbnail: true,
                quantize: true,
            },
        )
        .unwrap();
        assert_eq!(set.thumbnail_size, (40, 40));
        let decoded = decode(&set.thumbnail).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0)[3], 0, "top padding is transparent");
        assert_eq!(
            decoded.get_pixel(0, 20)[3],
            255,
            "the image is in the middle"
        );
    }

    #[test]
    fn small_images_are_upscaled_for_the_cover_only() {
        let png = gradient(50, 50, image::ImageFormat::Png);
        let set = make_cover(&png, &CoverOptions::default()).unwrap();
        assert_eq!(set.cover_size, (204, 204));
        assert_eq!(set.thumbnail_size, (40, 40));
        let tiny = gradient(20, 10, image::ImageFormat::Png);
        let set = make_cover(&tiny, &CoverOptions::default()).unwrap();
        assert_eq!(set.thumbnail_size, (20, 10));
    }

    #[test]
    fn exact_palettes_keep_every_colour() {
        let image = RgbaImage::from_fn(8, 8, |x, y| {
            image::Rgba([x as u8 * 30, y as u8 * 30, 0, 255])
        });
        let png = encode_png(&image, true).unwrap();
        let back = decode(&png).unwrap().to_rgba8();
        assert_eq!(back.as_raw(), image.as_raw());
        let transparent = RgbaImage::from_fn(4, 4, |x, _| {
            image::Rgba([0, 0, 0, if x == 0 { 0 } else { 255 }])
        });
        let png = encode_png(&transparent, true).unwrap();
        let back = decode(&png).unwrap().to_rgba8();
        assert_eq!(back.as_raw(), transparent.as_raw());
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(matches!(decode(b"nope"), Err(Error::Image(_))));
        assert!(matches!(
            make_cover(b"nope", &CoverOptions::default()),
            Err(Error::Image(_))
        ));
    }

    #[test]
    fn folder_designs_are_well_formed_and_distinct() {
        for rows in [FOLDER_DESIGN, BACK_DESIGN, HOME_DESIGN] {
            for row in rows {
                assert_eq!(row.len(), DESIGN_WIDTH, "{row}");
                assert!(row.bytes().all(|b| b".#o+".contains(&b)), "{row}");
            }
        }
        for c in ('A'..='Z').chain(['#', '?']) {
            for row in font_glyph(c) {
                assert_eq!(row.len(), 5, "{c}: {row}");
                assert!(row.bytes().all(|b| b".#".contains(&b)), "{c}: {row}");
            }
        }
        let a = folder_icon(&FolderImage::Letter('A')).unwrap();
        let b = folder_icon(&FolderImage::Letter('B')).unwrap();
        let back = folder_icon(&FolderImage::Back).unwrap();
        assert_ne!(a, b);
        assert_ne!(back, folder_icon(&FolderImage::Home).unwrap());
        assert_eq!(png_dimensions(&back), Some((COVER_WIDTH, COVER_HEIGHT)));
        assert_eq!(
            png_dimensions(&folder_thumbnail(&FolderImage::Back).unwrap()),
            Some((THUMB_SIZE, THUMB_SIZE))
        );
        // The arrow's tip is the outline colour at design cell (2, 8); the corner is background.
        let design = folder_design(&FolderImage::Back);
        assert_eq!(design[8][2], Ink::Outline);
        assert_eq!(design[0][0], Ink::Background);
        // A letter's shadow sits one cell right and below its strokes.
        let letter = folder_design(&FolderImage::Letter('L'));
        assert_eq!(letter[1][4], Ink::Fill);
        assert!(letter.iter().flatten().any(|i| *i == Ink::Outline));
    }

    /// `cargo test -p cartload-library folder_tiles_to_disk -- --ignored` writes the folder
    /// tiles to `target/folder-tiles/` for a look.
    #[test]
    #[ignore]
    fn folder_tiles_to_disk() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/folder-tiles");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, image) in [
            ("folder", FolderImage::Folder),
            ("back", FolderImage::Back),
            ("home", FolderImage::Home),
            ("letter", FolderImage::Letter('A')),
            ("letter-m", FolderImage::Letter('M')),
            ("letter-s", FolderImage::Letter('S')),
            ("letter-q", FolderImage::Letter('Q')),
            ("number", FolderImage::Number),
        ] {
            std::fs::write(
                dir.join(format!("{name}.png")),
                folder_icon(&image).unwrap(),
            )
            .unwrap();
            std::fs::write(
                dir.join(format!("{name}_small.png")),
                folder_thumbnail(&image).unwrap(),
            )
            .unwrap();
        }
    }
}
