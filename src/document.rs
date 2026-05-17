//! Top-level PDF document handle.

use crate::error::{Error, Result};
use crate::page::Page;
use std::path::Path;

/// An opened PDF document, parsed by `lopdf`.
///
/// `Document` owns the underlying byte buffer and exposes per-page access.
/// Each [`Page`] borrows from the document, so the document must remain
/// alive while pages are used.
pub struct Document {
    pub(crate) inner: lopdf::Document,
    pub(crate) page_ids: Vec<lopdf::ObjectId>,
}

impl Document {
    /// Open a PDF from a path on disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let inner = lopdf::Document::load(path)?;
        Self::from_lopdf(inner)
    }

    /// Open a PDF from an in-memory byte slice.
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self> {
        let inner = lopdf::Document::load_mem(bytes.as_ref())?;
        Self::from_lopdf(inner)
    }

    fn from_lopdf(inner: lopdf::Document) -> Result<Self> {
        let page_ids: Vec<_> = inner.get_pages().into_values().collect();
        Ok(Self { inner, page_ids })
    }

    /// Total page count.
    pub fn num_pages(&self) -> usize {
        self.page_ids.len()
    }

    /// Borrow page at `index` (0-based).
    pub fn page(&self, index: usize) -> Result<Page<'_>> {
        if index >= self.page_ids.len() {
            return Err(Error::PageOutOfBounds(index));
        }
        Ok(Page::new(self, index))
    }

    /// Iterator over all pages in document order.
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
