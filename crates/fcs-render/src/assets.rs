//! Self-contained project-owned assets and restricted decoders for Render conformance.
//!
//! No function reads a path, network resource, system font, platform color service, or expected
//! raster. The encoder and font builder produce the fixed inputs; independent decode functions
//! validate the checked-in bytes before exposing pixels/outlines to the reference rasterizer.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;

use image::{
    ColorType, ImageEncoder, ImageFormat, ImageReader, Limits,
    codecs::{png::PngEncoder, webp::WebPEncoder},
};

const FONT_CHECKSUM_MAGIC: u32 = 0xb1b0_afba;

pub const PNG_PIXELS: [u8; 16] = [
    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
];
pub const WEBP_PIXELS: [u8; 16] = [
    0, 255, 255, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 0, 255, 255,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetError {
    CapabilityMissing,
    DecodeFailed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
    pub linear_premultiplied: Vec<[f64; 4]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutlinePoint {
    pub x: i16,
    pub y: i16,
    pub on_curve: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphOutline {
    pub contours: Vec<Vec<OutlinePoint>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestFont {
    pub units_per_em: u16,
    pub advances: Vec<u16>,
    pub left_side_bearings: Vec<i16>,
    pub cmap: BTreeMap<u32, u16>,
    pub glyphs: Vec<GlyphOutline>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub x_advance: f64,
    pub y_advance: f64,
    pub x_offset: f64,
    pub y_offset: f64,
}

pub fn encode_test_png() -> Vec<u8> {
    let mut output = Vec::new();
    PngEncoder::new(Cursor::new(&mut output))
        .write_image(&PNG_PIXELS, 2, 2, ColorType::Rgba8.into())
        .expect("fixed PNG encoding");
    output
}

pub fn encode_test_webp() -> Vec<u8> {
    let mut output = Vec::new();
    WebPEncoder::new_lossless(Cursor::new(&mut output))
        .write_image(&WEBP_PIXELS, 2, 2, ColorType::Rgba8.into())
        .expect("fixed lossless WebP encoding");
    output
}

pub fn decode_image(
    media_type: &str,
    color_space: &str,
    alpha: &str,
    bytes: &[u8],
) -> Result<DecodedImage, AssetError> {
    let format = match media_type {
        "image/png" => {
            check_png(bytes, color_space)?;
            ImageFormat::Png
        }
        "image/webp" => {
            check_webp(bytes)?;
            ImageFormat::WebP
        }
        _ => return Err(AssetError::CapabilityMissing),
    };
    if !matches!(color_space, "srgb" | "linear-srgb")
        || !matches!(alpha, "straight" | "premultiplied")
    {
        return Err(AssetError::DecodeFailed);
    }

    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| AssetError::DecodeFailed)?;
    let rgba = image.to_rgba8();
    let mut linear_premultiplied = Vec::with_capacity(rgba.len() / 4);
    for pixel in rgba.as_raw().chunks_exact(4) {
        let encoded_alpha = f64::from(pixel[3]) / 255.0;
        let encoded = [
            f64::from(pixel[0]) / 255.0,
            f64::from(pixel[1]) / 255.0,
            f64::from(pixel[2]) / 255.0,
        ];
        let straight_encoded = if alpha == "straight" {
            encoded
        } else if encoded_alpha == 0.0 {
            if encoded.iter().any(|component| component.to_bits() != 0) {
                return Err(AssetError::DecodeFailed);
            }
            [0.0; 3]
        } else {
            [
                encoded[0] / encoded_alpha,
                encoded[1] / encoded_alpha,
                encoded[2] / encoded_alpha,
            ]
        };
        let linear = straight_encoded.map(|component| {
            if color_space == "srgb" {
                srgb_to_linear(component)
            } else {
                component
            }
        });
        linear_premultiplied.push([
            linear[0] * encoded_alpha,
            linear[1] * encoded_alpha,
            linear[2] * encoded_alpha,
            encoded_alpha,
        ]);
    }
    Ok(DecodedImage {
        width: rgba.width(),
        height: rgba.height(),
        rgba8: rgba.into_raw(),
        linear_premultiplied,
    })
}

fn srgb_to_linear(value: f64) -> f64 {
    if value == 0.0 || value == 1.0 {
        return value;
    }
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn check_png(bytes: &[u8], color_space: &str) -> Result<(), AssetError> {
    if bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return Err(AssetError::DecodeFailed);
    }
    let mut position = 8usize;
    let mut first = true;
    let mut saw_iend = false;
    let mut seen = BTreeSet::new();
    while position < bytes.len() {
        let header_end = position.checked_add(8).ok_or(AssetError::DecodeFailed)?;
        if header_end > bytes.len() {
            return Err(AssetError::DecodeFailed);
        }
        let length =
            usize::try_from(be_u32(bytes, position)?).map_err(|_| AssetError::DecodeFailed)?;
        let chunk_end = header_end
            .checked_add(length)
            .and_then(|value| value.checked_add(4))
            .ok_or(AssetError::DecodeFailed)?;
        if chunk_end > bytes.len() {
            return Err(AssetError::DecodeFailed);
        }
        let kind = &bytes[position + 4..position + 8];
        let data = &bytes[header_end..header_end + length];
        if first && (kind != b"IHDR" || length != 13) {
            return Err(AssetError::DecodeFailed);
        }
        first = false;
        if matches!(kind, b"acTL" | b"fcTL" | b"fdAT" | b"iCCP") {
            return Err(AssetError::CapabilityMissing);
        }
        if matches!(kind, b"IHDR" | b"sRGB" | b"gAMA" | b"cHRM" | b"IEND")
            && !seen.insert(kind.to_vec())
        {
            return Err(AssetError::DecodeFailed);
        }
        match kind {
            b"IHDR" => validate_ihdr(data)?,
            b"sRGB" => {
                if length != 1 || color_space != "srgb" {
                    return Err(AssetError::DecodeFailed);
                }
            }
            b"gAMA" => {
                if length != 4 {
                    return Err(AssetError::DecodeFailed);
                }
                let expected = if color_space == "srgb" {
                    45_455
                } else {
                    100_000
                };
                if be_u32(data, 0)? != expected {
                    return Err(AssetError::DecodeFailed);
                }
            }
            b"cHRM" => {
                let expected = [
                    31_270, 32_900, 64_000, 33_000, 30_000, 60_000, 15_000, 6_000,
                ];
                if length != 32
                    || expected
                        .iter()
                        .enumerate()
                        .any(|(index, value)| be_u32(data, index * 4).ok() != Some(*value))
                {
                    return Err(AssetError::DecodeFailed);
                }
            }
            b"IEND" => {
                if length != 0 || chunk_end != bytes.len() {
                    return Err(AssetError::DecodeFailed);
                }
                saw_iend = true;
            }
            _ => {}
        }
        position = chunk_end;
        if saw_iend {
            break;
        }
    }
    saw_iend.then_some(()).ok_or(AssetError::DecodeFailed)
}

fn validate_ihdr(data: &[u8]) -> Result<(), AssetError> {
    let bit_depth = *data.get(8).ok_or(AssetError::DecodeFailed)?;
    let color_type = *data.get(9).ok_or(AssetError::DecodeFailed)?;
    let valid = match color_type {
        0 => matches!(bit_depth, 1 | 2 | 4 | 8 | 16),
        2 | 4 | 6 => matches!(bit_depth, 8 | 16),
        3 => matches!(bit_depth, 1 | 2 | 4 | 8),
        _ => false,
    };
    if !valid
        || data.get(10..13) != Some(&[0, 0, 0])
        || be_u32(data, 0)? == 0
        || be_u32(data, 4)? == 0
    {
        return Err(AssetError::DecodeFailed);
    }
    Ok(())
}

fn check_webp(bytes: &[u8]) -> Result<(), AssetError> {
    if bytes.len() < 20 || bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WEBP") {
        return Err(AssetError::DecodeFailed);
    }
    let declared = usize::try_from(le_u32(bytes, 4)?).map_err(|_| AssetError::DecodeFailed)?;
    if declared.checked_add(8) != Some(bytes.len()) {
        return Err(AssetError::DecodeFailed);
    }
    let mut position = 12usize;
    let mut saw_lossless = false;
    let mut saw_extended = false;
    while position < bytes.len() {
        let header_end = position.checked_add(8).ok_or(AssetError::DecodeFailed)?;
        if header_end > bytes.len() {
            return Err(AssetError::DecodeFailed);
        }
        let kind = &bytes[position..position + 4];
        let length =
            usize::try_from(le_u32(bytes, position + 4)?).map_err(|_| AssetError::DecodeFailed)?;
        let data_end = header_end
            .checked_add(length)
            .ok_or(AssetError::DecodeFailed)?;
        let chunk_end = data_end
            .checked_add(length % 2)
            .ok_or(AssetError::DecodeFailed)?;
        if chunk_end > bytes.len() {
            return Err(AssetError::DecodeFailed);
        }
        match kind {
            b"VP8L" => {
                if saw_lossless {
                    return Err(AssetError::DecodeFailed);
                }
                saw_lossless = true;
            }
            b"VP8X" => {
                if saw_extended || length != 10 {
                    return Err(AssetError::DecodeFailed);
                }
                saw_extended = true;
                let flags = bytes[header_end];
                if flags & (0x20 | 0x02) != 0 {
                    return Err(AssetError::CapabilityMissing);
                }
            }
            b"VP8 " | b"ANIM" | b"ANMF" | b"ICCP" => {
                return Err(AssetError::CapabilityMissing);
            }
            _ => {}
        }
        position = chunk_end;
    }
    if position != bytes.len() || !saw_lossless {
        return Err(AssetError::DecodeFailed);
    }
    Ok(())
}

/// Builds a deterministic seven-table TrueType font with glyph 0 empty and glyph 1 a centered
/// square. The table directory checksum for `head` is computed with checkSumAdjustment zero, then
/// the full-font adjustment is written so the complete big-endian u32 sum is `0xB1B0AFBA`.
pub fn build_test_font() -> Vec<u8> {
    let mut tables = [
        (*b"cmap", cmap_table()),
        (*b"glyf", glyf_table()),
        (*b"head", head_table()),
        (*b"hhea", hhea_table()),
        (*b"hmtx", hmtx_table()),
        (*b"loca", loca_table()),
        (*b"maxp", maxp_table()),
    ];
    tables.sort_by_key(|(tag, _)| *tag);
    let num_tables = tables.len() as u16;
    let directory_length = 12 + tables.len() * 16;
    let mut font = vec![0u8; directory_length];
    write_be_u32(&mut font, 0, 0x0001_0000);
    write_be_u16(&mut font, 4, num_tables);
    write_be_u16(&mut font, 6, 64);
    write_be_u16(&mut font, 8, 2);
    write_be_u16(&mut font, 10, num_tables * 16 - 64);

    let mut head_offset = None;
    for (index, (tag, table)) in tables.iter().enumerate() {
        pad_to(&mut font, 4);
        let offset = font.len();
        let record = 12 + index * 16;
        font[record..record + 4].copy_from_slice(tag);
        write_be_u32(&mut font, record + 4, table_checksum(tag, table));
        write_be_u32(&mut font, record + 8, offset as u32);
        write_be_u32(&mut font, record + 12, table.len() as u32);
        if tag == b"head" {
            head_offset = Some(offset);
        }
        font.extend_from_slice(table);
        pad_to(&mut font, 4);
    }
    let head_offset = head_offset.expect("head table");
    let adjustment = FONT_CHECKSUM_MAGIC.wrapping_sub(checksum(&font));
    write_be_u32(&mut font, head_offset + 8, adjustment);
    debug_assert_eq!(checksum(&font), FONT_CHECKSUM_MAGIC);
    font
}

pub fn decode_font(bytes: &[u8]) -> Result<TestFont, AssetError> {
    if bytes.len() < 12 || be_u32(bytes, 0)? != 0x0001_0000 {
        return Err(AssetError::CapabilityMissing);
    }
    if !bytes.len().is_multiple_of(4) || checksum(bytes) != FONT_CHECKSUM_MAGIC {
        return Err(AssetError::DecodeFailed);
    }
    let count = usize::from(be_u16(bytes, 4)?);
    if count != 7 || 12 + count * 16 > bytes.len() {
        return Err(AssetError::CapabilityMissing);
    }
    let mut tables = BTreeMap::new();
    let mut ranges = Vec::new();
    let mut previous_tag = None;
    for index in 0..count {
        let record = 12 + index * 16;
        let tag: [u8; 4] = bytes[record..record + 4]
            .try_into()
            .map_err(|_| AssetError::DecodeFailed)?;
        if previous_tag.is_some_and(|previous| previous >= tag) {
            return Err(AssetError::DecodeFailed);
        }
        previous_tag = Some(tag);
        let expected_checksum = be_u32(bytes, record + 4)?;
        let offset =
            usize::try_from(be_u32(bytes, record + 8)?).map_err(|_| AssetError::DecodeFailed)?;
        let length =
            usize::try_from(be_u32(bytes, record + 12)?).map_err(|_| AssetError::DecodeFailed)?;
        let end = offset.checked_add(length).ok_or(AssetError::DecodeFailed)?;
        if !offset.is_multiple_of(4) || end > bytes.len() || offset < 12 + count * 16 {
            return Err(AssetError::DecodeFailed);
        }
        let table = &bytes[offset..end];
        if table_checksum(&tag, table) != expected_checksum {
            return Err(AssetError::DecodeFailed);
        }
        ranges.push((offset, align_up(end, 4)));
        tables.insert(tag, table);
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(AssetError::DecodeFailed);
    }
    let required = [
        *b"cmap", *b"glyf", *b"head", *b"hhea", *b"hmtx", *b"loca", *b"maxp",
    ];
    if tables.keys().copied().collect::<Vec<_>>() != required {
        return Err(AssetError::CapabilityMissing);
    }

    let head = tables[b"head"];
    if head.len() != 54
        || be_u32(head, 0)? != 0x0001_0000
        || be_u32(head, 12)? != 0x5f0f_3cf5
        || be_i16(head, 50)? != 0
        || be_i16(head, 52)? != 0
    {
        return Err(AssetError::DecodeFailed);
    }
    let units_per_em = be_u16(head, 18)?;
    if !(16..=16_384).contains(&units_per_em) {
        return Err(AssetError::DecodeFailed);
    }
    let maxp = tables[b"maxp"];
    if maxp.len() != 32 || be_u32(maxp, 0)? != 0x0001_0000 {
        return Err(AssetError::DecodeFailed);
    }
    let glyph_count = usize::from(be_u16(maxp, 4)?);
    if glyph_count != 2 {
        return Err(AssetError::DecodeFailed);
    }
    let hhea = tables[b"hhea"];
    if hhea.len() != 36
        || be_u32(hhea, 0)? != 0x0001_0000
        || usize::from(be_u16(hhea, 34)?) != glyph_count
    {
        return Err(AssetError::DecodeFailed);
    }
    let (advances, left_side_bearings) = parse_hmtx(tables[b"hmtx"], glyph_count)?;
    let offsets = parse_loca(tables[b"loca"], glyph_count)?;
    let glyphs = parse_glyphs(tables[b"glyf"], &offsets)?;
    let cmap = parse_cmap(tables[b"cmap"])?;
    if cmap.get(&u32::from('A')) != Some(&1) {
        return Err(AssetError::DecodeFailed);
    }
    Ok(TestFont {
        units_per_em,
        advances,
        left_side_bearings,
        cmap,
        glyphs,
    })
}

pub fn shape_simple_ltr(font: &TestFont, text: &str) -> Result<Vec<ShapedGlyph>, AssetError> {
    let mut glyphs = Vec::new();
    for scalar in text.chars() {
        if scalar == '\0'
            || scalar.is_control()
            || matches!(scalar, '\u{2028}' | '\u{2029}' | '\u{200e}' | '\u{200f}')
        {
            return Err(AssetError::CapabilityMissing);
        }
        let glyph_id = *font
            .cmap
            .get(&u32::from(scalar))
            .filter(|glyph| **glyph != 0)
            .ok_or(AssetError::DecodeFailed)?;
        let advance = *font
            .advances
            .get(glyph_id as usize)
            .ok_or(AssetError::DecodeFailed)?;
        glyphs.push(ShapedGlyph {
            glyph_id: u32::from(glyph_id),
            x_advance: f64::from(advance) / f64::from(font.units_per_em),
            y_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        });
    }
    Ok(glyphs)
}

fn head_table() -> Vec<u8> {
    let mut table = vec![0u8; 54];
    write_be_u32(&mut table, 0, 0x0001_0000);
    write_be_u32(&mut table, 4, 0x0001_0000);
    write_be_u32(&mut table, 8, 0);
    write_be_u32(&mut table, 12, 0x5f0f_3cf5);
    write_be_u16(&mut table, 18, 1000);
    write_be_i16(&mut table, 36, -500);
    write_be_i16(&mut table, 38, -500);
    write_be_i16(&mut table, 40, 500);
    write_be_i16(&mut table, 42, 500);
    write_be_u16(&mut table, 46, 8);
    write_be_i16(&mut table, 48, 2);
    table
}

fn maxp_table() -> Vec<u8> {
    let mut table = vec![0u8; 32];
    write_be_u32(&mut table, 0, 0x0001_0000);
    write_be_u16(&mut table, 4, 2);
    write_be_u16(&mut table, 6, 4);
    write_be_u16(&mut table, 8, 1);
    write_be_u16(&mut table, 14, 1);
    table
}

fn hhea_table() -> Vec<u8> {
    let mut table = vec![0u8; 36];
    write_be_u32(&mut table, 0, 0x0001_0000);
    write_be_i16(&mut table, 4, 500);
    write_be_i16(&mut table, 6, -500);
    write_be_u16(&mut table, 10, 1000);
    write_be_i16(&mut table, 12, -500);
    write_be_i16(&mut table, 14, 500);
    write_be_i16(&mut table, 16, 500);
    write_be_i16(&mut table, 18, 1);
    write_be_u16(&mut table, 34, 2);
    table
}

fn hmtx_table() -> Vec<u8> {
    let mut table = Vec::with_capacity(8);
    put_be_u16(&mut table, 1000);
    put_be_i16(&mut table, 0);
    put_be_u16(&mut table, 1000);
    put_be_i16(&mut table, -500);
    table
}

fn cmap_table() -> Vec<u8> {
    let mut table = Vec::with_capacity(44);
    put_be_u16(&mut table, 0);
    put_be_u16(&mut table, 1);
    put_be_u16(&mut table, 3);
    put_be_u16(&mut table, 1);
    put_be_u32(&mut table, 12);
    put_be_u16(&mut table, 4);
    put_be_u16(&mut table, 32);
    put_be_u16(&mut table, 0);
    put_be_u16(&mut table, 4);
    put_be_u16(&mut table, 4);
    put_be_u16(&mut table, 1);
    put_be_u16(&mut table, 0);
    put_be_u16(&mut table, 0x0041);
    put_be_u16(&mut table, 0xffff);
    put_be_u16(&mut table, 0);
    put_be_u16(&mut table, 0x0041);
    put_be_u16(&mut table, 0xffff);
    put_be_i16(&mut table, -64);
    put_be_i16(&mut table, 1);
    put_be_u16(&mut table, 0);
    put_be_u16(&mut table, 0);
    table
}

fn loca_table() -> Vec<u8> {
    let mut table = Vec::with_capacity(6);
    put_be_u16(&mut table, 0);
    put_be_u16(&mut table, 0);
    put_be_u16(&mut table, 17);
    table
}

fn glyf_table() -> Vec<u8> {
    let mut table = Vec::with_capacity(34);
    put_be_i16(&mut table, 1);
    put_be_i16(&mut table, -500);
    put_be_i16(&mut table, -500);
    put_be_i16(&mut table, 500);
    put_be_i16(&mut table, 500);
    put_be_u16(&mut table, 3);
    put_be_u16(&mut table, 0);
    table.extend_from_slice(&[1, 1, 1, 1]);
    for delta in [-500, 1000, 0, -1000] {
        put_be_i16(&mut table, delta);
    }
    for delta in [-500, 0, 1000, 0] {
        put_be_i16(&mut table, delta);
    }
    table
}

fn parse_hmtx(bytes: &[u8], count: usize) -> Result<(Vec<u16>, Vec<i16>), AssetError> {
    if bytes.len() != count * 4 {
        return Err(AssetError::DecodeFailed);
    }
    let mut advances = Vec::with_capacity(count);
    let mut bearings = Vec::with_capacity(count);
    for index in 0..count {
        advances.push(be_u16(bytes, index * 4)?);
        bearings.push(be_i16(bytes, index * 4 + 2)?);
    }
    Ok((advances, bearings))
}

fn parse_loca(bytes: &[u8], glyph_count: usize) -> Result<Vec<usize>, AssetError> {
    if bytes.len() != (glyph_count + 1) * 2 {
        return Err(AssetError::DecodeFailed);
    }
    let offsets: Vec<_> = (0..=glyph_count)
        .map(|index| be_u16(bytes, index * 2).map(|value| usize::from(value) * 2))
        .collect::<Result<_, _>>()?;
    if offsets.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(AssetError::DecodeFailed);
    }
    Ok(offsets)
}

fn parse_glyphs(bytes: &[u8], offsets: &[usize]) -> Result<Vec<GlyphOutline>, AssetError> {
    if offsets.last().copied() != Some(bytes.len()) {
        return Err(AssetError::DecodeFailed);
    }
    let mut glyphs = Vec::with_capacity(offsets.len() - 1);
    for range in offsets.windows(2) {
        if range[0] == range[1] {
            glyphs.push(GlyphOutline {
                contours: Vec::new(),
            });
            continue;
        }
        glyphs.push(parse_simple_glyph(&bytes[range[0]..range[1]])?);
    }
    Ok(glyphs)
}

fn parse_simple_glyph(bytes: &[u8]) -> Result<GlyphOutline, AssetError> {
    if bytes.len() < 14 {
        return Err(AssetError::DecodeFailed);
    }
    let contour_count = be_i16(bytes, 0)?;
    if contour_count <= 0 {
        return Err(if contour_count < 0 {
            AssetError::CapabilityMissing
        } else {
            AssetError::DecodeFailed
        });
    }
    let contour_count = contour_count as usize;
    let mut position = 10usize;
    let mut ends = Vec::with_capacity(contour_count);
    for _ in 0..contour_count {
        ends.push(be_u16(bytes, position)? as usize);
        position += 2;
    }
    if ends.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(AssetError::DecodeFailed);
    }
    let point_count = ends.last().copied().ok_or(AssetError::DecodeFailed)? + 1;
    let instruction_length = usize::from(be_u16(bytes, position)?);
    position = position
        .checked_add(2 + instruction_length)
        .ok_or(AssetError::DecodeFailed)?;
    if position > bytes.len() {
        return Err(AssetError::DecodeFailed);
    }
    let mut flags = Vec::with_capacity(point_count);
    while flags.len() < point_count {
        let flag = *bytes.get(position).ok_or(AssetError::DecodeFailed)?;
        position += 1;
        flags.push(flag);
        if flag & 0x08 != 0 {
            let repeats = usize::from(*bytes.get(position).ok_or(AssetError::DecodeFailed)?);
            position += 1;
            if flags.len() + repeats > point_count {
                return Err(AssetError::DecodeFailed);
            }
            flags.extend(std::iter::repeat_n(flag, repeats));
        }
    }
    let (xs, next) = decode_coordinates(bytes, position, &flags, true)?;
    let (ys, next) = decode_coordinates(bytes, next, &flags, false)?;
    if next != bytes.len() {
        return Err(AssetError::DecodeFailed);
    }
    let points: Vec<_> = xs
        .into_iter()
        .zip(ys)
        .zip(flags)
        .map(|((x, y), flag)| OutlinePoint {
            x,
            y,
            on_curve: flag & 1 != 0,
        })
        .collect();
    let mut contours = Vec::with_capacity(contour_count);
    let mut start = 0usize;
    for end in ends {
        contours.push(points[start..=end].to_vec());
        start = end + 1;
    }
    Ok(GlyphOutline { contours })
}

fn decode_coordinates(
    bytes: &[u8],
    mut position: usize,
    flags: &[u8],
    x_axis: bool,
) -> Result<(Vec<i16>, usize), AssetError> {
    let short_bit = if x_axis { 0x02 } else { 0x04 };
    let same_bit = if x_axis { 0x10 } else { 0x20 };
    let mut current = 0i32;
    let mut values = Vec::with_capacity(flags.len());
    for flag in flags {
        let delta = if flag & short_bit != 0 {
            let magnitude = i32::from(*bytes.get(position).ok_or(AssetError::DecodeFailed)?);
            position += 1;
            if flag & same_bit != 0 {
                magnitude
            } else {
                -magnitude
            }
        } else if flag & same_bit != 0 {
            0
        } else {
            let value = i32::from(be_i16(bytes, position)?);
            position += 2;
            value
        };
        current = current.checked_add(delta).ok_or(AssetError::DecodeFailed)?;
        values.push(i16::try_from(current).map_err(|_| AssetError::DecodeFailed)?);
    }
    Ok((values, position))
}

fn parse_cmap(bytes: &[u8]) -> Result<BTreeMap<u32, u16>, AssetError> {
    if bytes.len() < 12 || be_u16(bytes, 0)? != 0 {
        return Err(AssetError::DecodeFailed);
    }
    let count = usize::from(be_u16(bytes, 2)?);
    if count == 0 || 4 + count * 8 > bytes.len() {
        return Err(AssetError::DecodeFailed);
    }
    let mut candidates = Vec::new();
    for index in 0..count {
        let record = 4 + index * 8;
        let platform = be_u16(bytes, record)?;
        let encoding = be_u16(bytes, record + 2)?;
        let offset =
            usize::try_from(be_u32(bytes, record + 4)?).map_err(|_| AssetError::DecodeFailed)?;
        if platform != 3 || !matches!(encoding, 1 | 10) || offset + 2 > bytes.len() {
            continue;
        }
        let format = be_u16(bytes, offset)?;
        if (format == 12 && encoding == 10) || (format == 4 && matches!(encoding, 1 | 10)) {
            candidates.push((if format == 12 { 0 } else { 1 }, encoding, offset, format));
        }
    }
    candidates.sort_unstable();
    let (_, _, offset, format) = candidates
        .first()
        .copied()
        .ok_or(AssetError::CapabilityMissing)?;
    match format {
        4 => parse_cmap4(bytes, offset),
        12 => Err(AssetError::CapabilityMissing),
        _ => unreachable!(),
    }
}

fn parse_cmap4(bytes: &[u8], offset: usize) -> Result<BTreeMap<u32, u16>, AssetError> {
    let length = usize::from(be_u16(bytes, offset + 2)?);
    let table = bytes
        .get(offset..offset + length)
        .ok_or(AssetError::DecodeFailed)?;
    let segment_count = usize::from(be_u16(table, 6)?) / 2;
    if segment_count == 0 || 16 + segment_count * 8 > table.len() {
        return Err(AssetError::DecodeFailed);
    }
    let end_codes = 14;
    let start_codes = end_codes + segment_count * 2 + 2;
    let deltas = start_codes + segment_count * 2;
    let ranges = deltas + segment_count * 2;
    let mut cmap = BTreeMap::new();
    for segment in 0..segment_count {
        let end = be_u16(table, end_codes + segment * 2)?;
        let start = be_u16(table, start_codes + segment * 2)?;
        let delta = be_i16(table, deltas + segment * 2)? as i32;
        let range = be_u16(table, ranges + segment * 2)?;
        if start > end {
            return Err(AssetError::DecodeFailed);
        }
        for code in start..=end {
            if code == 0xffff {
                continue;
            }
            let glyph = if range == 0 {
                ((i32::from(code) + delta) & 0xffff) as u16
            } else {
                return Err(AssetError::CapabilityMissing);
            };
            cmap.insert(u32::from(code), glyph);
        }
    }
    Ok(cmap)
}

fn table_checksum(tag: &[u8; 4], table: &[u8]) -> u32 {
    if tag == b"head" {
        let mut zeroed = table.to_vec();
        if zeroed.len() >= 12 {
            zeroed[8..12].fill(0);
        }
        checksum(&zeroed)
    } else {
        checksum(table)
    }
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

fn be_u16(bytes: &[u8], offset: usize) -> Result<u16, AssetError> {
    Ok(u16::from_be_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(AssetError::DecodeFailed)?
            .try_into()
            .map_err(|_| AssetError::DecodeFailed)?,
    ))
}

fn be_i16(bytes: &[u8], offset: usize) -> Result<i16, AssetError> {
    Ok(i16::from_be_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(AssetError::DecodeFailed)?
            .try_into()
            .map_err(|_| AssetError::DecodeFailed)?,
    ))
}

fn be_u32(bytes: &[u8], offset: usize) -> Result<u32, AssetError> {
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(AssetError::DecodeFailed)?
            .try_into()
            .map_err(|_| AssetError::DecodeFailed)?,
    ))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, AssetError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(AssetError::DecodeFailed)?
            .try_into()
            .map_err(|_| AssetError::DecodeFailed)?,
    ))
}

fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

fn pad_to(bytes: &mut Vec<u8>, alignment: usize) {
    bytes.resize(align_up(bytes.len(), alignment), 0);
}

fn put_be_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn put_be_i16(bytes: &mut Vec<u8>, value: i16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn put_be_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn write_be_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_be_i16(bytes: &mut [u8], offset: usize, value: i16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_be_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}
