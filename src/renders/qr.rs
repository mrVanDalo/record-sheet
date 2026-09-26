//! QR-code encoding for the bottom-right corner of the sheet.

use crate::RecordError;

/// An encoded QR code: a square matrix of modules, row 0 at the top.
#[derive(Debug, Clone, PartialEq)]
pub struct QrMatrix {
    /// Module count per side (`version * 4 + 17`).
    pub size: usize,
    /// Dark-module flags in row-major order, top row first, `size * size` entries.
    pub modules: Vec<bool>,
}

/// Encode `text` into a QR matrix (byte mode auto-chosen by the crate,
/// error correction level M, smallest fitting version).
pub fn encode_qr(text: &str) -> Result<QrMatrix, RecordError> {
    let code = fast_qr::qr::QRBuilder::new(text)
        .ecl(fast_qr::ECL::M)
        .build()
        .map_err(|error| RecordError::InvalidQrCode(error.to_string()))?;
    let size = code.size;
    let mut modules = Vec::with_capacity(size * size);
    for row in 0..size {
        for column in 0..size {
            modules.push(code[row][column].value());
        }
    }
    Ok(QrMatrix { size, modules })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_small_text_to_version_1() {
        let qr = encode_qr("hello").unwrap();
        assert_eq!(qr.size, 21);
        assert_eq!(qr.modules.len(), 441);
    }

    #[test]
    fn finder_patterns_present() {
        let qr = encode_qr("hello").unwrap();
        let at = |row: usize, column: usize| qr.modules[row * qr.size + column];
        // Corner pixels and inner corner of each 7x7 finder ring are dark in
        // every valid QR code; the separators right/below them stay light.
        assert!(at(0, 0));
        assert!(at(0, 6));
        assert!(at(6, 0));
        assert!(at(6, 6));
        assert!(at(0, 14));
        assert!(at(20, 0));
        assert!(!at(0, 7));
        assert!(!at(7, 0));
    }

    #[test]
    fn rejects_too_long_input() {
        assert!(matches!(
            encode_qr(&"x".repeat(10_000)),
            Err(RecordError::InvalidQrCode(_))
        ));
    }
}
