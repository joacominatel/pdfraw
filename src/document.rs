//! Top-level PDF document handle.

use crate::error::{Error, Result};
use crate::page::Page;
use std::path::Path;

/// An opened PDF document.
///
/// `Document` owns the parsed byte buffer; [`Page`]s borrow from it, so the
/// document must outlive every page it produces.
pub struct Document {
    pub(crate) inner: lopdf::Document,
    pub(crate) page_ids: Vec<lopdf::ObjectId>,
}

impl Document {
    /// Open and parse a PDF from a path on disk.
    ///
    /// # Errors
    ///
    /// - [`Error::Io`] if the file cannot be opened or read.
    /// - [`Error::Pdf`] if the bytes are not a valid PDF.
    /// - [`Error::Unsupported`] if the PDF is encrypted (password-protected
    ///   PDFs are not handled yet).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use pdfraw::Document;
    /// let doc = Document::open("invoice.pdf")?;
    /// println!("{} page(s)", doc.num_pages());
    /// # Ok::<_, pdfraw::Error>(())
    /// ```
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let inner = lopdf::Document::load(path)?;
        Self::from_lopdf(inner)
    }

    /// Parse a PDF from an in-memory byte slice.
    ///
    /// # Errors
    ///
    /// Same as [`Document::open`] without the I/O variant.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use pdfraw::Document;
    /// let bytes: Vec<u8> = std::fs::read("invoice.pdf")?;
    /// let doc = Document::from_bytes(&bytes)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self> {
        let inner = lopdf::Document::load_mem(bytes.as_ref())?;
        Self::from_lopdf(inner)
    }

    fn from_lopdf(inner: lopdf::Document) -> Result<Self> {
        if inner.is_encrypted() {
            return Err(Error::Unsupported(
                "encrypted PDF — password support is not implemented yet",
            ));
        }
        let page_ids: Vec<_> = inner.get_pages().into_values().collect();
        Ok(Self { inner, page_ids })
    }

    /// Total number of pages in this document.
    pub fn num_pages(&self) -> usize {
        self.page_ids.len()
    }

    /// Borrow the page at `index` (0-based).
    ///
    /// # Errors
    ///
    /// [`Error::PageOutOfBounds`] when `index >= self.num_pages()`.
    pub fn page(&self, index: usize) -> Result<Page<'_>> {
        if index >= self.page_ids.len() {
            return Err(Error::PageOutOfBounds(index));
        }
        Ok(Page::new(self, index))
    }

    /// Iterator over all pages in document order.
    ///
    /// Each iteration step calls [`Document::page`] internally, so callers
    /// see the same `Result` semantics.
    pub fn pages(&self) -> Pages<'_> {
        Pages { doc: self, next: 0 }
    }
}

/// Iterator returned by [`Document::pages`].
pub struct Pages<'doc> {
    doc: &'doc Document,
    next: usize,
}

impl<'doc> Iterator for Pages<'doc> {
    type Item = Result<Page<'doc>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.doc.num_pages() {
            return None;
        }
        let i = self.next;
        self.next += 1;
        Some(self.doc.page(i))
    }
}
