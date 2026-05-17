//! Library error type and result alias.
//!
//! [`Error`] is `#[non_exhaustive]` so future variants can be added without
//! a breaking change. Match against it with a wildcard arm.

use thiserror::Error;

/// Convenience alias for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur while parsing or extracting from a PDF.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// I/O failure while reading a file from disk.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Underlying `lopdf` parse error (malformed PDF structure, missing
    /// xref table, invalid object, etc.).
    #[error("PDF parse error: {0}")]
    Pdf(#[from] lopdf::Error),

    /// A page's content stream is malformed or references something the
    /// parser could not resolve.
    #[error("invalid content stream on page {page}: {reason}")]
    ContentStream {
        /// 0-based page index.
        page: usize,
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// A PDF feature was detected but is not implemented yet — currently
    /// returned for encrypted PDFs (no password support in this MVP).
    #[error("unsupported feature: {0}")]
    Unsupported(&'static str),

    /// Font decoding failed (bad CMap, unknown encoding, malformed widths).
    #[error("font decoding failed: {0}")]
    Font(String),

    /// `Document::page(i)` was called with `i >= num_pages()`.
    #[error("page index {0} out of bounds")]
    PageOutOfBounds(usize),
}
