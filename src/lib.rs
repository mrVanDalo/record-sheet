//! record-sheet: printable calendar record sheet PDF generator.

pub mod render;
pub mod sheet;

pub use render::render_pdf;
pub use sheet::Sheet;

use thiserror::Error;

/// Errors returned by the record-sheet library.
#[derive(Debug, Error)]
pub enum RecordError {
    #[error("invalid date: {0}")]
    InvalidDate(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience alias for results in this crate.
pub type Result<T, E = RecordError> = std::result::Result<T, E>;
