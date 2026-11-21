//! Texture pixels: decode the formats this game uses to RGBA, paste PNG images into them.
//!
//! Pixel rows are kept in the engine's order (first row = bottom of the image); PNG rows are top
//! first, so pasting flips them. Re-encoding uses RGBA32 (no lossy re-compression, no encoder).

use crate::Error;

pub const FMT_ALPHA8: i64 = 1;
pub const FMT_RGB24: i64 = 3;
pub const FMT_RGBA32: i64 = 4;
pub const FMT_ARGB32: i64 = 5;
pub const FMT_ETC_RGB4: i64 = 34;
pub const FMT_ETC2_RGBA8: i64 = 47;

/// Largest texture side we handle (the game's atlases are at most 2048).
pub const MAX_SIDE: usize = 8192;

/// Decode the first mip level to RGBA8 (engine row order).
pub fn decode(format: i64, width: usize, height: usize, data: &[u8]) -> Result<Vec<u8>, Error> {
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(format!("texture size {width}x{height} out of range"));
    }
    let n = width * height;
    let need = |bytes: usize| -> Result<(), Error> {
        (data.len() >= bytes)
            .then_some(())
            .ok_or_else(|| format!("texture data too short ({} < {bytes})", data.len()))
    };
    let mut out = vec![0u8; n * 4];
    match format {
        FMT_RGBA32 => {
            need(n * 4)?;
            out.copy_from_slice(&data[..n * 4]);
        }
        FMT_ARGB32 => {
            need(n * 4)?;
            for (o, p) in out.chunks_exact_mut(4).zip(data.chunks_exact(4)) {
                o.copy_from_slice(&[p[1], p[2], p[3], p[0]]);
            }
        }
        FMT_RGB24 => {
            need(n * 3)?;
            for (o, p) in out.chunks_exact_mut(4).zip(data.chunks_exact(3)) {
                o.copy_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        FMT_ALPHA8 => {
            need(n)?;
            for (o, a) in out.chunks_exact_mut(4).zip(data) {
                o.copy_from_slice(&[255, 255, 255, *a]);
            }
        }
        FMT_ETC_RGB4 | FMT_ETC2_RGBA8 => {
            let mut px = vec![0u32; n];
            let r = if format == FMT_ETC_RGB4 {
                texture2ddecoder::decode_etc1(data, width, height, &mut px)
            } else {
                texture2ddecoder::decode_etc2_rgba8(data, width, height, &mut px)
            };
            r.map_err(|e| format!("decode: {e}"))?;
            // The decoder's u32 holds B, G, R, A in little-endian byte order.
            for (o, p) in out.chunks_exact_mut(4).zip(px) {
                let [b, g, r, a] = p.to_le_bytes();
                o.copy_from_slice(&[r, g, b, a]);
            }
        }
        f => return Err(format!("texture format {f} not supported")),
    }
    Ok(out)
}

/// A decoded PNG: width, height, RGBA8 rows top first.
pub fn decode_png(bytes: &[u8]) -> Result<(usize, usize, Vec<u8>), Error> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(
        png::Transformations::normalize_to_color8() | png::Transformations::ALPHA,
    );
    let mut r = dec.read_info().map_err(|e| format!("png: {e}"))?;
    let (w, h) = (r.info().width as usize, r.info().height as usize);
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("png size {w}x{h} out of range"));
    }
    let mut buf = vec![0u8; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).map_err(|e| format!("png: {e}"))?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        c => return Err(format!("png colour type {c:?} after expansion")),
    };
    if rgba.len() != w * h * 4 {
        return Err("png: unexpected buffer size".into());
    }
    Ok((w, h, rgba))
}

/// Paste a top-first RGBA image into an engine-order RGBA texture at the sprite rectangle whose
/// lower-left corner is (`x`, `y`) in texture coordinates (y up). The image replaces those pixels
/// entirely, alpha included.
#[allow(clippy::too_many_arguments)]
pub fn paste(
    tex: &mut [u8],
    tw: usize,
    th: usize,
    img: &[u8],
    iw: usize,
    ih: usize,
    x: usize,
    y: usize,
) -> Result<(), Error> {
    if x + iw > tw || y + ih > th {
        return Err(format!(
            "sprite {iw}x{ih} at ({x},{y}) does not fit a {tw}x{th} texture"
        ));
    }
    for row in 0..ih {
        let src = &img[row * iw * 4..(row + 1) * iw * 4];
        let ty = y + (ih - 1 - row); // PNG top row = highest texture row
        let at = (ty * tw + x) * 4;
        tex[at..at + iw * 4].copy_from_slice(src);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_of(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(rgba).unwrap();
        out
    }

    #[test]
    fn png_round_trip_and_paste_flips_rows() {
        // 2x2 image: top row red, bottom row green.
        let img = [
            255, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
        ];
        let (w, h, px) = decode_png(&png_of(2, 2, &img)).unwrap();
        assert_eq!((w, h, px.as_slice()), (2, 2, &img[..]));
        let mut tex = vec![0u8; 4 * 4 * 4];
        paste(&mut tex, 4, 4, &px, 2, 2, 1, 2).unwrap();
        // Engine row 2 (lower row of the sprite) is the PNG's bottom row (green).
        let at = |x: usize, y: usize| &tex[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4];
        assert_eq!(at(1, 2), [0, 255, 0, 255]);
        assert_eq!(at(1, 3), [255, 0, 0, 255]);
        assert_eq!(at(0, 0), [0, 0, 0, 0], "outside the rect untouched");
        assert!(paste(&mut tex, 4, 4, &px, 2, 2, 3, 3).is_err());
    }

    #[test]
    fn decodes_simple_formats() {
        let rgb = [1, 2, 3, 4, 5, 6];
        assert_eq!(
            decode(FMT_RGB24, 2, 1, &rgb).unwrap(),
            [1, 2, 3, 255, 4, 5, 6, 255]
        );
        assert_eq!(decode(FMT_ALPHA8, 1, 1, &[7]).unwrap(), [255, 255, 255, 7]);
        assert!(decode(FMT_RGBA32, 2, 2, &[0; 4]).is_err());
        assert!(decode(99, 1, 1, &[0; 4]).is_err());
        // An all-zero ETC2 RGBA8 block decodes without error to a 4x4 image.
        assert_eq!(decode(FMT_ETC2_RGBA8, 4, 4, &[0; 16]).unwrap().len(), 64);
    }
}
