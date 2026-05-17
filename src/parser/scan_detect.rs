//! Heuristic detection of scanned (image-only) pages.
//!
//! Implementation lands in phase 6. Stub returns `Ok(false)`.

use crate::error::Result;
use crate::page::Page;

/// Returns `true` when the page appears to be a scanned image with no real
/// text content.
pub(crate) fn is_scanned(_page: &Page<'_>) -> Result<bool> {
    Ok(false)
}
