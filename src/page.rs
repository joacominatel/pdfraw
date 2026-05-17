//! Per-page handle exposing chars, words, and text extraction.

use crate::char::Char;
use crate::document::Document;
use crate::error::Result;
use crate::text::options::TextOptions;
use crate::word::{Word, WordOptions};
use std::sync::OnceLock;

/// A borrowed view into one page of a [`Document`].
pub struct Page<'doc> {
    doc: &'doc Document,
    index: usize,
    chars_cache: OnceLock<Vec<Char>>,
    metrics_cache: OnceLock<PageMetrics>,
}

/// Geometric metadata about a page.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PageMetrics {
    /// Width in points.
    pub width: f32,
    /// Height in points.
    pub height: f32,
    /// Rotation in degrees (0/90/180/270).
    pub rotation: i16,
}

impl<'doc> Page<'doc> {
    pub(crate) fn new(doc: &'doc Document, index: usize) -> Self {
        Self {
            doc,
            index,
            chars_cache: OnceLock::new(),
            metrics_cache: OnceLock::new(),
        }
    }

    /// 0-based page index in document order.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Underlying [`Document`] this page belongs to.
    pub fn document(&self) -> &'doc Document {
        self.doc
    }

    /// Internal: the `lopdf` object ID of this page.
    pub(crate) fn page_id(&self) -> lopdf::ObjectId {
        self.doc.page_ids[self.index]
    }

    fn metrics(&self) -> &PageMetrics {
        self.metrics_cache.get_or_init(|| {
            crate::parser::lopdf_backend::page_metrics(&self.doc.inner, self.page_id())
                .unwrap_or(PageMetrics { width: 612.0, height: 792.0, rotation: 0 })
        })
    }

    /// Page width in points.
    pub fn width(&self) -> f32 {
        self.metrics().width
    }

    /// Page height in points.
    pub fn height(&self) -> f32 {
        self.metrics().height
    }

    /// Page rotation in degrees (`0`, `90`, `180`, `270`).
    pub fn rotation(&self) -> i16 {
        self.metrics().rotation
    }

    /// Lazily parse and cache the page's chars.
    ///
    /// On parse failure the error is returned without being cached, so a
    /// subsequent call will re-attempt extraction.
    pub fn chars(&self) -> Result<&[Char]> {
        if let Some(v) = self.chars_cache.get() {
            return Ok(v.as_slice());
        }
        let v = crate::parser::lopdf_backend::extract_chars(self)?;
        // Race: if another thread populated it first, our value is dropped.
        let _ = self.chars_cache.set(v);
        Ok(self.chars_cache.get().expect("just set").as_slice())
    }

    /// Cluster the page's chars into words using `opts`.
    pub fn words(&self, opts: &WordOptions) -> Result<Vec<Word>> {
        let chars = self.chars()?;
        Ok(crate::text::extractor::extract_words(chars, opts))
    }

    /// Naive concatenated text: words separated by `' '`, lines by `'\n'`.
    pub fn extract_text(&self) -> Result<String> {
        let chars = self.chars()?;
        Ok(crate::text::extractor::extract_text_simple(chars))
    }

    /// Layout-preserving text — replicates pdfplumber's `extract_text(layout=True)`.
    pub fn extract_text_layout(&self, opts: &TextOptions) -> Result<String> {
        let chars = self.chars()?;
        Ok(crate::text::extractor::extract_text_layout(chars, opts))
    }

    /// Return `Ok(true)` if the page has no text operators but contains an
    /// image XObject covering more than half of its area.
    pub fn is_scanned(&self) -> Result<bool> {
        crate::parser::scan_detect::is_scanned(self)
    }
}
