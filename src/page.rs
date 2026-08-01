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
            crate::parser::lopdf_backend::page_metrics(&self.doc.inner, self.page_id(), self.index)
                .unwrap_or(PageMetrics {
                    width: 612.0,
                    height: 792.0,
                    rotation: 0,
                })
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

    /// Lazily parse and cache the page's positioned glyphs.
    ///
    /// The first call walks the content stream; subsequent calls return the
    /// cached slice in O(1).
    ///
    /// # Errors
    ///
    /// - [`Error::Pdf`](crate::Error::Pdf) if the page dictionary or content
    ///   stream cannot be read.
    /// - [`Error::ContentStream`](crate::Error::ContentStream) if the
    ///   content stream is malformed.
    ///
    /// On error nothing is cached, so a retry will re-attempt the parse.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use pdfraw::prelude::*;
    /// # fn run() -> Result<()> {
    /// let doc = Document::open("invoice.pdf")?;
    /// for c in doc.page(0)?.chars()? {
    ///     println!("{} at ({}, {})", c.text, c.x0, c.top);
    /// }
    /// # Ok(()) }
    /// ```
    pub fn chars(&self) -> Result<&[Char]> {
        if let Some(cached) = self.chars_cache.get() {
            return Ok(cached.as_slice());
        }
        let parsed = crate::parser::lopdf_backend::extract_chars(self)?;
        // `set` returns `Err(parsed)` only when another caller already
        // populated the cell first. In that case our `parsed` is dropped
        // and we read whatever the winner stored — both walks of the same
        // content stream produce the same chars, so the result is
        // observationally identical.
        let _ = self.chars_cache.set(parsed);
        // Invariant: after the get-or-set sequence above, the cell is
        // always populated. `expect` here documents that post-condition
        // rather than masking a runtime failure.
        Ok(self
            .chars_cache
            .get()
            .expect("chars_cache populated by either set() above or a concurrent caller")
            .as_slice())
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

    /// Return `Ok(true)` when the page emits no text-showing operators but
    /// does draw image-like content (any `Do` XObject invocation or an
    /// inline `BI` image) — i.e. OCR is probably what you want.
    ///
    /// A page that draws nothing at all returns `false`. See
    /// `docs/decisions/0008-widened-scan-detection.md` for why the check
    /// does not descend into the XObject to confirm `/Subtype /Image`.
    pub fn is_scanned(&self) -> Result<bool> {
        crate::parser::scan_detect::is_scanned(self)
    }
}
