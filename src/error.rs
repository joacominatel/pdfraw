//! Library error type.

use thiserror::Error;

/// Library result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur while parsing or extracting from a PDF.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O failure while reading a file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Underlying `lopdf` parse error.
    #[error("PDF parse error: {0}")]
    Pdf(#[from] lopdf::Error),

    /// Content stream malformed on a specific page.
    #[error("invalid content stream on page {page}: {reason}")]
    ContentStream {
        /// 0-based page index.
        page: usize,
        /// Reason for the failure.
        reason: String,
    },

    /// A feature is recognized in the PDF but not yet supported.
    #[error("unsupported feature: {0}")]
    Unsupported(&'static str),

    /// Failure decoding a font (CMap, encoding, widths, etc).
    #[error("font decoding failed: {0}")]
    Font(String),

    /// Page index out of bounds.
    #[error("page index {0} out of bounds")]
    PageOutOfBounds(usize),
}
