//! # pdfraw
//!
//! Layout-preserving PDF text extraction in pure Rust, inspired by
//! [pdfplumber](https://github.com/jsvine/pdfplumber).
//!
//! The core goal is to extract text from PDFs while preserving the original
//! spatial layout: column alignment, line breaks, right-aligned numbers in
//! invoices and credit notes, multi-page documents, etc.
//!
//! ## Quick start
//!
//! ```no_run
//! use pdfraw::prelude::*;
//!
//! # fn main() -> Result<()> {
//! let doc = Document::open("invoice.pdf")?;
//! for page in doc.pages() {
//!     let page = page?;
//!     let text = page.extract_text_layout(&TextOptions::pdfplumber_defaults())?;
//!     println!("--- page {} ---\n{}", page.index(), text);
//! }
//! # Ok(()) }
//! ```
//!
//! ## Modules
//!
//! - [`document`]: top-level [`Document`] and metadata.
//! - [`page`]: per-page [`Page`] handle with text extraction methods.
//! - [`mod@char`]: positioned glyph [`Char`] (post-decoding).
//! - [`word`]: clustered [`Word`] with options.
//! - [`text`]: clustering, word extraction, layout reconstruction.
//! - [`error`]: error type and result alias.
//! - [`prelude`]: convenient re-exports.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod char;
pub mod document;
pub mod error;
pub mod geom;
pub mod page;
pub mod prelude;
pub mod text;
pub mod word;

// Internal parsing layer — implementation detail, not part of the stable API.
pub(crate) mod parser;

pub use crate::char::Char;
pub use crate::document::Document;
pub use crate::error::{Error, Result};
pub use crate::page::Page;
pub use crate::text::options::{TextOptions, TextOptionsBuilder};
pub use crate::word::{Word, WordOptions};
