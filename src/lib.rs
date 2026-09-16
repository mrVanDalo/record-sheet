//! record-sheet: printable calendar record sheet PDF generator.

pub mod config;
pub mod models;
pub mod renders;

pub use config::render::{MonthCellConfig, PageConfig, RenderConfig, WeekCellConfig};
pub use models::sheet::{
    Language, Month, Position, Row, RowGroup, RowGroupKind, Sheet, Week, WeekHeader, Year,
};
pub use renders::pdf::render_pdf;

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
