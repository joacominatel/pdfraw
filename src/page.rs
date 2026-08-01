//! Per-page handle exposing chars, words, and text extraction.

use crate::char::Char;
use crate::document::Document;
use crate::error::Result;
use crate::geom::Matrix;
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
///
/// The media box is stored as the file declares it; `/Rotate` is applied on
/// the way out, so [`Self::display_width`] and [`Self::display_height`] are
/// the dimensions a reader would show.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PageMetrics {
    /// `/MediaBox` width in points, before `/Rotate`.
    pub media_width: f32,
    /// `/MediaBox` height in points, before `/Rotate`.
    pub media_height: f32,
    /// x of the `/MediaBox` lower-left corner. Usually `0`, but not always.
    pub origin_x: f32,
    /// y of the `/MediaBox` lower-left corner.
    pub origin_y: f32,
    /// Rotation in degrees, normalized to `0`, `90`, `180`, or `270`.
    pub rotation: i16,
}

impl PageMetrics {
    /// `true` when `/Rotate` swaps the page's width and height.
    fn is_quarter_turn(&self) -> bool {
        self.rotation == 90 || self.rotation == 270
    }

    /// Width as displayed, after `/Rotate`.
    pub(crate) fn display_width(&self) -> f32 {
        if self.is_quarter_turn() {
            self.media_height
        } else {
            self.media_width
        }
    }

    /// Height as displayed, after `/Rotate`.
    pub(crate) fn display_height(&self) -> f32 {
        if self.is_quarter_turn() {
            self.media_width
        } else {
            self.media_height
        }
    }

    /// Transform from raw PDF user space into displayed page space, both
    /// still bottom-up. Composing this after the CTM is what makes the
    /// `/MediaBox` origin and `/Rotate` apply to glyph positions, not just
    /// to the page dimensions.
    ///
    /// Two steps. First the lower-left corner of the media box is moved to
    /// `(0, 0)` — it is not always there, and ignoring it shifted every
    /// glyph by that corner. Then `/Rotate` turns the page clockwise, so for
    /// 90° the corner ends up at the top-left of what the reader shows.
    pub(crate) fn page_transform(&self) -> Matrix {
        let to_origin = Matrix::translation(-self.origin_x, -self.origin_y);
        let (w, h) = (self.media_width, self.media_height);
        let rotate = match self.rotation {
            90 => Matrix::new(0.0, -1.0, 1.0, 0.0, 0.0, w),
            180 => Matrix::new(-1.0, 0.0, 0.0, -1.0, w, h),
            270 => Matrix::new(0.0, 1.0, -1.0, 0.0, h, 0.0),
            _ => Matrix::IDENTITY,
        };
        to_origin.then(rotate)
    }
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

    pub(crate) fn metrics(&self) -> &PageMetrics {
        self.metrics_cache.get_or_init(|| {
            crate::parser::lopdf_backend::page_metrics(&self.doc.inner, self.page_id(), self.index)
                .unwrap_or(PageMetrics {
                    media_width: 612.0,
                    media_height: 792.0,
                    origin_x: 0.0,
                    origin_y: 0.0,
                    rotation: 0,
                })
        })
    }

    /// Page width in points, as displayed.
    ///
    /// A page with `/Rotate 90` or `/Rotate 270` reports its media box
    /// height here, because that is the edge a reader shows horizontally.
    pub fn width(&self) -> f32 {
        self.metrics().display_width()
    }

    /// Page height in points, as displayed. See [`Self::width`].
    pub fn height(&self) -> f32 {
        self.metrics().display_height()
    }

    /// Page rotation in degrees, always one of `0`, `90`, `180`, `270`.
    ///
    /// The value is normalized: a negative `/Rotate` wraps into range, and a
    /// `/Rotate` that is not a multiple of 90 is reported as `0`.
    ///
    /// Rotation is already applied to [`Self::width`], [`Self::height`] and
    /// to every [`Char`] coordinate, so callers do not need to apply it
    /// themselves — this is here to describe the source page.
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

    /// The page's chars with double-struck copies removed.
    ///
    /// A generator that fakes bold by drawing the same string twice, a
    /// fraction of a point apart, leaves two glyphs per character. See
    /// [`crate::text::dedupe::dedupe_chars`] for the rule and for the
    /// tolerance's meaning; `1.0` is the value pdfplumber defaults to.
    ///
    /// This allocates a new `Vec` rather than touching the cache, so
    /// [`Self::chars`] keeps reporting what the file actually contains.
    pub fn dedupe_chars(&self, tolerance: f32) -> Result<Vec<Char>> {
        Ok(crate::text::dedupe::dedupe_chars(self.chars()?, tolerance))
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
    ///
    /// Honours [`TextOptions::dedupe_tolerance`]: when set, double-struck
    /// glyphs are removed before layout, which is what stops fake bold from
    /// coming out as `SSAALLDDOO`.
    pub fn extract_text_layout(&self, opts: &TextOptions) -> Result<String> {
        let chars = self.chars()?;
        Ok(match opts.dedupe_tolerance {
            Some(t) => {
                let deduped = crate::text::dedupe::dedupe_chars(chars, t);
                crate::text::extractor::extract_text_layout(&deduped, opts)
            }
            None => crate::text::extractor::extract_text_layout(chars, opts),
        })
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
