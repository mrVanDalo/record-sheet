//! Decoding of PNG logos for the title band.

use crate::RecordError;

/// A decoded logo image, normalized to 8-bit RGB with an optional alpha plane.
#[derive(Debug, Clone, PartialEq)]
pub struct LogoImage {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Opaque RGB samples, `width * height * 3` bytes.
    pub rgb: Vec<u8>,
    /// Optional alpha samples, `width * height` bytes, present whenever the
    /// PNG (after expansion) carries an alpha channel.
    pub alpha: Option<Vec<u8>>,
    /// Trim window of visible (non-transparent) pixels: inclusive left/top,
    /// exclusive right/bottom.
    pub trim_left: u32,
    pub trim_top: u32,
    pub trim_right: u32,
    pub trim_bottom: u32,
}

/// Decode a PNG image into an 8-bit RGB [`LogoImage`].
///
/// Any palette, low bit depth, or 16-bit input is expanded/stripped by the
/// decoder, so the result is always 8-bit samples.
pub fn decode_png(bytes: &[u8]) -> Result<LogoImage, RecordError> {
    let invalid = |error: png::DecodingError| RecordError::InvalidPng(error.to_string());
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(invalid)?;
    let (color, depth) = reader.output_color_type();
    let frame_size = reader
        .output_buffer_size()
        .ok_or_else(|| RecordError::InvalidPng("image buffer too large".to_string()))?;
    let mut frame = vec![0u8; frame_size];
    let info = reader.next_frame(&mut frame).map_err(invalid)?;
    if depth != png::BitDepth::Eight {
        return Err(RecordError::InvalidPng(format!(
            "unsupported bit depth {depth:?}"
        )));
    }

    let width = info.width;
    let height = info.height;
    let pixel_count = width as usize * height as usize;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    let mut alpha = Vec::new();
    match color {
        png::ColorType::Grayscale => {
            for &gray in &frame {
                rgb.extend_from_slice(&[gray, gray, gray]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pair in frame.chunks_exact(2) {
                rgb.extend_from_slice(&[pair[0], pair[0], pair[0]]);
                alpha.push(pair[1]);
            }
        }
        png::ColorType::Rgb => rgb = frame,
        png::ColorType::Rgba => {
            for pixel in frame.chunks_exact(4) {
                rgb.extend_from_slice(&pixel[..3]);
                alpha.push(pixel[3]);
            }
        }
        // `normalize_to_color8` expands palettes before the frame is read;
        // `Indexed` can never appear here.
        png::ColorType::Indexed => {
            return Err(RecordError::InvalidPng(
                "unexpected palette image".to_string(),
            ));
        }
    }
    let alpha = (!alpha.is_empty()).then_some(alpha);
    // Bounding box of visible (non-transparent) pixels: inclusive left/top,
    // exclusive right/bottom. Without an alpha channel (or with no visible
    // pixel at all) the trim window is the full raster.
    let mut trim_left = width;
    let mut trim_top = height;
    let mut trim_right = 0;
    let mut trim_bottom = 0;
    if let Some(alpha) = &alpha {
        for y in 0..height {
            for x in 0..width {
                if alpha[y as usize * width as usize + x as usize] != 0 {
                    trim_left = trim_left.min(x);
                    trim_right = trim_right.max(x + 1);
                    trim_top = trim_top.min(y);
                    trim_bottom = trim_bottom.max(y + 1);
                }
            }
        }
        if trim_right == 0 {
            // All-transparent: fall back to the full raster.
            trim_left = 0;
            trim_top = 0;
            trim_right = width;
            trim_bottom = height;
        }
    } else {
        trim_left = 0;
        trim_top = 0;
        trim_right = width;
        trim_bottom = height;
    }
    Ok(LogoImage {
        width,
        height,
        rgb,
        alpha,
        trim_left,
        trim_top,
        trim_right,
        trim_bottom,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1x1 RGBA PNG (200, 30, 40, 128), raw bytes, no binary fixture file.
    const PNG_1X1_RGBA: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0xd, 0xa, 0x1a, 0xa, 0x0, 0x0, 0x0, 0xd, 0x49, 0x48, 0x44, 0x52,
        0x0, 0x0, 0x0, 0x1, 0x0, 0x0, 0x0, 0x1, 0x8, 0x6, 0x0, 0x0, 0x0, 0x1f, 0x15, 0xc4, 0x89,
        0x0, 0x0, 0x0, 0xd, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x38, 0x21, 0xa7, 0xd1, 0x0,
        0x0, 0x4, 0x4f, 0x1, 0x8f, 0xd1, 0xc9, 0x9e, 0xa5, 0x0, 0x0, 0x0, 0x0, 0x49, 0x45, 0x4e,
        0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn decodes_1x1_rgba() {
        let logo = decode_png(PNG_1X1_RGBA).unwrap();
        assert_eq!(logo.width, 1);
        assert_eq!(logo.height, 1);
        assert_eq!(logo.rgb, vec![200, 30, 40]);
        assert_eq!(logo.alpha, Some(vec![128]));
        assert_eq!(
            (logo.trim_left, logo.trim_top, logo.trim_right, logo.trim_bottom),
            (0, 0, 1, 1)
        );
    }

    // 3x3 RGBA PNG with a fully transparent border and one opaque center
    // pixel (10, 20, 30, 255), raw bytes, no binary fixture file.
    const PNG_3X3_TRANSPARENT_BORDER: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0xd, 0xa, 0x1a, 0xa, 0x0, 0x0, 0x0, 0xd, 0x49, 0x48, 0x44, 0x52,
        0x0, 0x0, 0x0, 0x3, 0x0, 0x0, 0x0, 0x3, 0x8, 0x6, 0x0, 0x0, 0x0, 0x56, 0x28, 0xb5,
        0xbf, 0x0, 0x0, 0x0, 0x11, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60, 0x40, 0x7, 0x5c,
        0x22, 0x72, 0xff, 0x31, 0x4, 0x1, 0x16, 0xb1, 0x1, 0x3c, 0x16, 0xe, 0xe2, 0x52, 0x0, 0x0,
        0x0, 0x0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn trims_transparent_border() {
        let logo = decode_png(PNG_3X3_TRANSPARENT_BORDER).unwrap();
        assert_eq!(logo.width, 3);
        assert_eq!(logo.height, 3);
        assert_eq!(
            (logo.trim_left, logo.trim_top, logo.trim_right, logo.trim_bottom),
            (1, 1, 2, 2)
        );
    }

    #[test]
    fn garbage_bytes_fail() {
        let error = decode_png(b"not a png").unwrap_err();
        assert!(
            matches!(error, crate::RecordError::InvalidPng(_)),
            "expected InvalidPng, got {error:?}"
        );
    }
}