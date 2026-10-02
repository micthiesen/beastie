use std::{f64::consts::PI, fs, io::Write, path::Path};

use anyhow::{Context, Result, bail, ensure};
use image::{
    DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba, RgbaImage,
    imageops::{FilterType, crop_imm, resize},
};

const SOURCE: &str = "steam/assets/source/key-art.png";
const APP_ICON_SOURCE: &str = "steam/assets/source/app-icon.jpg";
const PROVENANCE: &str = "steam/assets/provenance.json";
const EXPECTED_SOURCE_SHA256: &str =
    "3f70dccf99902ed94be863b7e8d0a25e78fbab6a997db89f5a72b6a27c831bbe";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Format {
    Png,
    Jpeg,
}

#[derive(Clone, Copy, Debug)]
struct Asset {
    file: &'static str,
    width: u32,
    height: u32,
    format: Format,
    logo: bool,
}

const ASSETS: [Asset; 10] = [
    Asset {
        file: "header.png",
        width: 920,
        height: 430,
        format: Format::Png,
        logo: true,
    },
    Asset {
        file: "small-capsule.png",
        width: 462,
        height: 174,
        format: Format::Png,
        logo: true,
    },
    Asset {
        file: "main-capsule.png",
        width: 1232,
        height: 706,
        format: Format::Png,
        logo: true,
    },
    Asset {
        file: "vertical-capsule.png",
        width: 748,
        height: 896,
        format: Format::Png,
        logo: true,
    },
    Asset {
        file: "library-capsule.png",
        width: 600,
        height: 900,
        format: Format::Png,
        logo: true,
    },
    Asset {
        file: "library-hero.png",
        width: 3840,
        height: 1240,
        format: Format::Png,
        logo: false,
    },
    Asset {
        file: "library-header.png",
        width: 920,
        height: 430,
        format: Format::Png,
        logo: true,
    },
    Asset {
        file: "library-logo.png",
        width: 1280,
        height: 720,
        format: Format::Png,
        logo: true,
    },
    Asset {
        file: "shortcut.png",
        width: 256,
        height: 256,
        format: Format::Png,
        logo: false,
    },
    Asset {
        file: "app-icon.jpg",
        width: 184,
        height: 184,
        format: Format::Jpeg,
        logo: false,
    },
];

const LOGO_FILL: [u8; 4] = [255, 239, 179, 255];
const LOGO_OUTLINE: [u8; 4] = [31, 88, 116, 255];
const LOGO_SHADOW: [u8; 4] = [4, 18, 32, 220];
const LOGO_PIXELS: [[u8; 5]; 7] = [
    [0b11110, 0b10000, 0b11100, 0b10000, 0b11110], // B
    [0b11111, 0b10000, 0b11110, 0b10000, 0b11111], // E
    [0b01110, 0b10001, 0b10001, 0b11111, 0b10001], // A
    [0b01111, 0b10000, 0b01110, 0b00001, 0b11110], // S
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100], // T
    [0b11111, 0b00100, 0b00100, 0b00100, 0b11111], // I
    [0b11111, 0b10000, 0b11110, 0b10000, 0b11111], // E
];

pub fn build(root: &Path) -> Result<()> {
    let source_path = root.join(SOURCE);
    let source_bytes = fs::read(&source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?;
    let source_sha256 = sha256_hex(&source_bytes);
    ensure!(
        source_sha256 == EXPECTED_SOURCE_SHA256,
        "{} SHA-256 changed: expected {EXPECTED_SOURCE_SHA256}, got {source_sha256}",
        source_path.display()
    );
    let source = image::load_from_memory_with_format(&source_bytes, ImageFormat::Png)
        .context("Steam key art must be a valid PNG")?;
    ensure!(
        source.dimensions() == (1672, 941),
        "Steam key art must be 1672x941, got {}x{}",
        source.width(),
        source.height()
    );

    let assets_root = root.join("steam/assets");
    for asset in ASSETS {
        let path = assets_root.join(asset.file);
        if asset.file == "library-logo.png" {
            write_png(&path, &logo_canvas(asset.width, asset.height))?;
            continue;
        }
        let mut image = cover_crop(&source, asset.width, asset.height);
        if asset.file != "app-icon.jpg" && asset.file != "shortcut.png" {
            grade(&mut image);
        }
        if asset.logo {
            draw_logo(&mut image);
        }
        match asset.format {
            Format::Png => write_png(&path, &image)?,
            Format::Jpeg => fs::copy(root.join(APP_ICON_SOURCE), &path)
                .with_context(|| format!("failed to copy {}", APP_ICON_SOURCE))
                .map(|_| ())?,
        }
    }
    write_provenance(&root.join(PROVENANCE), &source_sha256)?;
    check(root)
}

pub fn check(root: &Path) -> Result<()> {
    let source_path = root.join(SOURCE);
    let source_bytes = fs::read(&source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?;
    ensure!(
        sha256_hex(&source_bytes) == EXPECTED_SOURCE_SHA256,
        "{} does not match recorded source SHA-256",
        source_path.display()
    );
    let source = image::load_from_memory_with_format(&source_bytes, ImageFormat::Png)
        .context("Steam key art must be a valid PNG")?;
    ensure!(
        source.dimensions() == (1672, 941),
        "Steam key art dimensions changed"
    );
    let app_icon_bytes = fs::read(root.join(APP_ICON_SOURCE))
        .context("missing approved steam/assets/source/app-icon.jpg")?;

    for asset in ASSETS {
        check_asset(
            &root.join("steam/assets").join(asset.file),
            asset,
            &source,
            &app_icon_bytes,
        )?;
    }
    let provenance = fs::read_to_string(root.join(PROVENANCE))
        .context("missing steam/assets/provenance.json; run cargo xtask store-assets build")?;
    ensure!(
        provenance.contains(EXPECTED_SOURCE_SHA256) && provenance.contains("OpenAI-generated"),
        "Steam asset provenance must record source SHA-256 and OpenAI provenance"
    );
    Ok(())
}

fn check_asset(
    path: &Path,
    asset: Asset,
    source: &DynamicImage,
    app_icon_bytes: &[u8],
) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("missing {}", path.display()))?;
    ensure!(!bytes.is_empty(), "{} is empty", path.display());
    match asset.format {
        Format::Png => ensure!(
            bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "{} is not PNG",
            path.display()
        ),
        Format::Jpeg => ensure!(
            bytes.starts_with(&[0xff, 0xd8]) && bytes.ends_with(&[0xff, 0xd9]),
            "{} is not JPEG",
            path.display()
        ),
    }
    let decoded = if asset.format == Format::Png {
        Some(
            image::load_from_memory_with_format(&bytes, ImageFormat::Png)
                .with_context(|| format!("failed to decode {}", path.display()))?,
        )
    } else {
        ensure!(
            jpeg_dimensions(&bytes)? == (asset.width, asset.height),
            "{} has incorrect dimensions",
            path.display()
        );
        ensure!(
            bytes == app_icon_bytes,
            "{} does not match the approved app-icon source",
            path.display()
        );
        None
    };
    if let Some(decoded) = decoded {
        ensure!(
            decoded.dimensions() == (asset.width, asset.height),
            "{} has incorrect dimensions",
            path.display()
        );
        if asset.logo {
            ensure!(
                has_logo_color(&decoded),
                "{} is missing the BEASTIE logo",
                path.display()
            );
        }
        if asset.file == "library-logo.png" {
            ensure!(
                decoded.to_rgba8().pixels().any(|pixel| pixel[3] == 0),
                "library-logo.png must have transparency"
            );
        }
        let expected = expected_asset(source, asset);
        ensure!(
            decoded.to_rgba8() == expected,
            "{} does not match deterministic output; run cargo xtask store-assets build",
            path.display()
        );
    }
    Ok(())
}

fn expected_asset(source: &DynamicImage, asset: Asset) -> RgbaImage {
    if asset.file == "library-logo.png" {
        return logo_canvas(asset.width, asset.height);
    }
    let mut image = cover_crop(source, asset.width, asset.height);
    if asset.file != "shortcut.png" {
        grade(&mut image);
    }
    if asset.logo {
        draw_logo(&mut image);
    }
    image
}

fn cover_crop(source: &DynamicImage, target_width: u32, target_height: u32) -> RgbaImage {
    let (source_width, source_height) = source.dimensions();
    let scale = (target_width as f64 / source_width as f64)
        .max(target_height as f64 / source_height as f64);
    let resized_width = (source_width as f64 * scale).ceil() as u32;
    let resized_height = (source_height as f64 * scale).ceil() as u32;
    let resized = resize(source, resized_width, resized_height, FilterType::Lanczos3);
    let x = (resized_width - target_width) / 2;
    let y = (resized_height - target_height) / 2;
    crop_imm(&resized, x, y, target_width, target_height).to_image()
}

fn grade(image: &mut RgbaImage) {
    let height = image.height().max(1);
    let width = image.width().max(1);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let vignette_x = ((x as f32 / width as f32) - 0.5).abs() * 18.0;
        let shade = 25.0 * (1.0 - y as f32 / height as f32) + vignette_x;
        for channel in 0..3 {
            let contrast = (f32::from(pixel[channel]) - 128.0) * 1.10 + 128.0;
            pixel[channel] = (contrast - shade).clamp(0.0, 255.0) as u8;
        }
        pixel[2] = pixel[2].saturating_add(7);
    }
}

fn logo_canvas(width: u32, height: u32) -> RgbaImage {
    let mut canvas = ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    draw_logo(&mut canvas);
    canvas
}

fn draw_logo(image: &mut RgbaImage) {
    let total_columns = 41_u32;
    let unit = ((image.width() * 72 / 100) / total_columns)
        .max(1)
        .min((image.height() * 26 / 100) / 7)
        .max(1);
    let width = total_columns * unit;
    let height = 7 * unit;
    let left = (image.width() - width) / 2;
    let top = ((image.height() * 12 / 100).min(image.height().saturating_sub(height + 2))).max(2);
    for (letter_index, glyph) in LOGO_PIXELS.iter().enumerate() {
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..5_u32 {
                if bits & (1 << (4 - column)) != 0 {
                    let x = left + (letter_index as u32 * 6 + column) * unit;
                    let y = top + row as u32 * unit;
                    block(image, x + unit / 3, y + unit / 3, unit, LOGO_SHADOW);
                    block(
                        image,
                        x.saturating_sub(1),
                        y.saturating_sub(1),
                        unit + 2,
                        LOGO_OUTLINE,
                    );
                    block(image, x, y, unit, LOGO_FILL);
                }
            }
        }
    }
}

fn block(image: &mut RgbaImage, x: u32, y: u32, size: u32, color: [u8; 4]) {
    for yy in y..y.saturating_add(size).min(image.height()) {
        for xx in x..x.saturating_add(size).min(image.width()) {
            image.put_pixel(xx, yy, Rgba(color));
        }
    }
}

fn has_logo_color(image: &DynamicImage) -> bool {
    image
        .to_rgba8()
        .pixels()
        .filter(|pixel| pixel.0 == LOGO_FILL)
        .count()
        >= 24
}

fn write_png(path: &Path, image: &RgbaImage) -> Result<()> {
    image
        .save_with_format(path, ImageFormat::Png)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[allow(dead_code)]
fn write_jpeg(path: &Path, image: &RgbaImage) -> Result<()> {
    let mut bytes = Vec::new();
    encode_baseline_jpeg(&mut bytes, image)?;
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

#[allow(dead_code)]
fn jpeg_dimensions(bytes: &[u8]) -> Result<(u32, u32)> {
    ensure!(bytes.starts_with(&[0xff, 0xd8]), "missing JPEG SOI marker");
    let mut index = 2;
    while index + 4 <= bytes.len() {
        ensure!(bytes[index] == 0xff, "invalid JPEG marker");
        while index < bytes.len() && bytes[index] == 0xff {
            index += 1;
        }
        let marker = bytes[index];
        index += 1;
        if marker == 0xd9 {
            break;
        }
        let length = usize::from(u16::from_be_bytes([bytes[index], bytes[index + 1]]));
        ensure!(
            length >= 2 && index + length <= bytes.len(),
            "invalid JPEG segment length"
        );
        if (0xc0..=0xc3).contains(&marker) {
            ensure!(length >= 8, "invalid JPEG frame header");
            return Ok((
                u32::from(u16::from_be_bytes([bytes[index + 5], bytes[index + 6]])),
                u32::from(u16::from_be_bytes([bytes[index + 3], bytes[index + 4]])),
            ));
        }
        index += length;
    }
    bail!("JPEG is missing a frame header")
}

fn write_provenance(path: &Path, source_sha256: &str) -> Result<()> {
    let contents = format!(
        "{{\n  \"version\": 1,\n  \"source\": {{\n    \"path\": \"source/key-art.png\",\n    \"dimensions\": \"1672x941\",\n    \"sha256\": \"{source_sha256}\",\n    \"provenance\": \"OpenAI-generated no-text key art\"\n  }},\n  \"build\": \"cargo xtask store-assets build (offline, deterministic)\",\n  \"logo\": \"code-native 5x5 chunky BEASTIE glyphs\"\n}}\n"
    );
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

// Minimal, deterministic baseline JFIF encoder. It keeps this offline pipeline portable while
// Cargo's intentionally PNG-only `image` feature set is used for the rest of the artwork.
fn encode_baseline_jpeg(writer: &mut Vec<u8>, image: &RgbaImage) -> Result<()> {
    ensure!(
        image.width() <= u16::MAX.into() && image.height() <= u16::MAX.into(),
        "JPEG dimensions exceed baseline limits"
    );
    writer.write_all(&[0xff, 0xd8])?;
    marker(
        writer,
        0xe0,
        &[b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0],
    )?;
    let mut dqt = Vec::with_capacity(130);
    dqt.extend([0, 1]);
    dqt.extend(std::iter::repeat_n(8, 64));
    dqt.extend(std::iter::repeat_n(9, 64));
    marker(writer, 0xdb, &dqt)?;
    marker(
        writer,
        0xc0,
        &[
            8,
            (image.height() >> 8) as u8,
            image.height() as u8,
            (image.width() >> 8) as u8,
            image.width() as u8,
            3,
            1,
            0x11,
            0,
            2,
            0x11,
            1,
            3,
            0x11,
            1,
        ],
    )?;
    marker(writer, 0xc4, &dht_data())?;
    marker(writer, 0xda, &[3, 1, 0, 2, 0x11, 3, 0x11, 0, 63, 0])?;
    let tables = huffman_tables();
    let mut bits = BitWriter::new(writer);
    let mut previous = [0_i16; 3];
    for by in (0..image.height()).step_by(8) {
        for bx in (0..image.width()).step_by(8) {
            for (component, prior) in previous.iter_mut().enumerate() {
                let table_index = if component == 0 { 0 } else { 2 };
                let block = dct_block(image, bx, by, component);
                let dc = block[0];
                let diff = dc - *prior;
                *prior = dc;
                write_huffman(&mut bits, &tables[table_index], magnitude_category(diff))?;
                write_magnitude(&mut bits, diff)?;
                let mut run = 0_u8;
                for &index in ZIGZAG.iter().skip(1) {
                    let value = block[index];
                    if value == 0 {
                        run += 1;
                        continue;
                    }
                    while run >= 16 {
                        write_huffman(&mut bits, &tables[table_index + 1], 0xf0)?;
                        run -= 16;
                    }
                    let category = magnitude_category(value);
                    write_huffman(&mut bits, &tables[table_index + 1], (run << 4) | category)?;
                    write_magnitude(&mut bits, value)?;
                    run = 0;
                }
                if run > 0 {
                    write_huffman(&mut bits, &tables[table_index + 1], 0)?;
                }
            }
        }
    }
    bits.finish()?;
    writer.write_all(&[0xff, 0xd9])?;
    Ok(())
}

fn marker(writer: &mut Vec<u8>, code: u8, data: &[u8]) -> Result<()> {
    writer.write_all(&[0xff, code])?;
    writer.write_all(&((data.len() as u16 + 2).to_be_bytes()))?;
    writer.write_all(data)?;
    Ok(())
}

type HuffmanTable = [(u16, u8); 256];

fn dht_data() -> Vec<u8> {
    let mut result = Vec::new();
    for (class_id, counts, values) in HUFFMAN_SPECS {
        result.push(class_id);
        result.extend(counts);
        result.extend(values);
    }
    result
}

fn huffman_tables() -> [HuffmanTable; 4] {
    let mut result = [[(0, 0); 256]; 4];
    for (table, (_, counts, values)) in result.iter_mut().zip(HUFFMAN_SPECS) {
        let mut code = 0_u16;
        let mut value_index = 0;
        for (length, &count) in counts.iter().enumerate() {
            for _ in 0..count {
                table[values[value_index] as usize] = (code, (length + 1) as u8);
                value_index += 1;
                code += 1;
            }
            code <<= 1;
        }
    }
    result
}

const HUFFMAN_SPECS: [(u8, [u8; 16], &[u8]); 4] = [
    (
        0x00,
        [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    ),
    (
        0x10,
        [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d],
        &[
            1, 2, 3, 0, 4, 0x11, 5, 0x12, 0x21, 0x31, 0x41, 6, 0x13, 0x51, 0x61, 7, 0x22, 0x71,
            0x14, 0x32, 0x81, 0x91, 0xa1, 8, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24,
            0x33, 0x62, 0x72, 0x82, 9, 0xa, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
            0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47,
            0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65,
            0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83,
            0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98,
            0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4,
            0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9,
            0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4,
            0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
            0xf9, 0xfa,
        ],
    ),
    (
        0x01,
        [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    ),
    (
        0x11,
        [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77],
        &[
            0, 1, 2, 3, 0x11, 4, 5, 0x21, 0x31, 6, 0x12, 0x41, 0x51, 7, 0x61, 0x71, 0x13, 0x22,
            0x32, 0x81, 8, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 9, 0x23, 0x33, 0x52, 0xf0, 0x15,
            0x62, 0x72, 0xd1, 0xa, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a,
            0x26, 0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45,
            0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63,
            0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79,
            0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95,
            0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa,
            0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6,
            0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1,
            0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5,
            0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
        ],
    ),
];

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

fn dct_block(image: &RgbaImage, block_x: u32, block_y: u32, component: usize) -> [i16; 64] {
    let mut values = [[0_f64; 8]; 8];
    for (y, row) in values.iter_mut().enumerate() {
        for (x, value) in row.iter_mut().enumerate() {
            let pixel = image.get_pixel(
                (block_x + x as u32).min(image.width() - 1),
                (block_y + y as u32).min(image.height() - 1),
            );
            let red = f64::from(pixel[0]);
            let green = f64::from(pixel[1]);
            let blue = f64::from(pixel[2]);
            *value = match component {
                0 => 0.299 * red + 0.587 * green + 0.114 * blue - 128.0,
                1 => -0.168736 * red - 0.331264 * green + 0.5 * blue,
                _ => 0.5 * red - 0.418688 * green - 0.081312 * blue,
            };
        }
    }
    let mut result = [0_i16; 64];
    for v in 0..8 {
        for u in 0..8 {
            let mut sum = 0.0;
            for (y, row) in values.iter().enumerate() {
                for (x, value) in row.iter().enumerate() {
                    sum += *value
                        * ((2 * x + 1) as f64 * u as f64 * PI / 16.0).cos()
                        * ((2 * y + 1) as f64 * v as f64 * PI / 16.0).cos();
                }
            }
            let scale = if u == 0 { 1.0 / 2.0_f64.sqrt() } else { 1.0 }
                * if v == 0 { 1.0 / 2.0_f64.sqrt() } else { 1.0 }
                / 4.0;
            result[v * 8 + u] = (sum * scale / if component == 0 { 8.0 } else { 9.0 })
                .round()
                .clamp(-1023.0, 1023.0) as i16;
        }
    }
    result
}

fn magnitude_category(value: i16) -> u8 {
    if value == 0 {
        0
    } else {
        (16 - value.unsigned_abs().leading_zeros()) as u8
    }
}
fn write_magnitude(writer: &mut BitWriter<'_>, value: i16) -> Result<()> {
    let category = magnitude_category(value);
    if category > 0 {
        let bits = if value < 0 {
            ((1_i32 << category) - 1 + i32::from(value)) as u16
        } else {
            value as u16
        };
        writer.write(bits, category)?;
    }
    Ok(())
}
fn write_huffman(writer: &mut BitWriter<'_>, table: &HuffmanTable, value: u8) -> Result<()> {
    let (bits, length) = table[value as usize];
    ensure!(length > 0, "missing JPEG Huffman code for {value:#x}");
    writer.write(bits, length)
}

struct BitWriter<'a> {
    output: &'a mut Vec<u8>,
    buffer: u32,
    count: u8,
}
impl<'a> BitWriter<'a> {
    fn new(output: &'a mut Vec<u8>) -> Self {
        Self {
            output,
            buffer: 0,
            count: 0,
        }
    }
    fn write(&mut self, bits: u16, count: u8) -> Result<()> {
        self.buffer = (self.buffer << count) | u32::from(bits);
        self.count += count;
        while self.count >= 8 {
            let byte = (self.buffer >> (self.count - 8)) as u8;
            self.output.write_all(&[byte])?;
            if byte == 0xff {
                self.output.write_all(&[0])?;
            }
            self.count -= 8;
            self.buffer &= (1_u32 << self.count).saturating_sub(1);
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<()> {
        if self.count > 0 {
            self.write((1 << (8 - self.count)) - 1, 8 - self.count)?;
        }
        Ok(())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let hash = sha256(bytes);
    let mut result = String::with_capacity(64);
    for byte in hash {
        use std::fmt::Write as _;
        write!(&mut result, "{byte:02x}").expect("string write cannot fail");
    }
    result
}
fn sha256(bytes: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut data = bytes.to_vec();
    let bit_len = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend(bit_len.to_be_bytes());
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in data.as_chunks::<64>().0.iter() {
        let mut w = [0_u32; 64];
        for (index, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes(
                chunk[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("chunk is 64 bytes"),
            );
        }
        for index in 16..64 {
            w[index] = w[index - 16]
                .wrapping_add(w[index - 7])
                .wrapping_add(
                    w[index - 15].rotate_right(7)
                        ^ w[index - 15].rotate_right(18)
                        ^ (w[index - 15] >> 3),
                )
                .wrapping_add(
                    w[index - 2].rotate_right(17)
                        ^ w[index - 2].rotate_right(19)
                        ^ (w[index - 2] >> 10),
                );
        }
        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }
    let mut out = [0; 32];
    for (index, word) in state.iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cover_crop_fills_requested_dimensions() {
        let source = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(8, 4, Rgba([1, 2, 3, 255])));
        assert_eq!(cover_crop(&source, 4, 4).dimensions(), (4, 4));
        assert_eq!(cover_crop(&source, 16, 4).dimensions(), (16, 4));
    }
    #[test]
    fn logo_layout_is_visible_on_thumbnail() {
        let mut image = ImageBuffer::from_pixel(462, 174, Rgba([0, 0, 0, 255]));
        draw_logo(&mut image);
        assert!(has_logo_color(&DynamicImage::ImageRgba8(image)));
    }
    #[test]
    fn jpeg_encoder_writes_requested_dimensions() {
        let image = ImageBuffer::from_pixel(16, 16, Rgba([20, 50, 100, 255]));
        let mut bytes = Vec::new();
        encode_baseline_jpeg(&mut bytes, &image).expect("encode JPEG");
        assert_eq!(jpeg_dimensions(&bytes).expect("parse JPEG"), (16, 16));
    }
    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
