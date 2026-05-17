//! Heuristic detection of scanned (image-only) pages.
//!
//! A page is considered scanned when:
//! 1. its content stream has zero `BT`/`ET` text blocks (no real text), and
//! 2. it contains at least one `Do` operator referencing an XObject of
//!    subtype `/Image`.
//!
//! This catches the common case where a PDF wraps a JPEG of a scanned
//! invoice and gives the caller a chance to route to OCR.

use crate::error::{Error, Result};
use crate::page::Page;
use lopdf::content::Content;
use lopdf::Object;

/// Returns `true` when the page appears to be a scanned image with no real
/// text content.
pub(crate) fn is_scanned(page: &Page<'_>) -> Result<bool> {
    let doc = &page.document().inner;
    let page_id = page.page_id();
    let raw = doc.get_page_content(page_id).map_err(|e| Error::ContentStream {
        page: page.index(),
        reason: format!("get_page_content: {e}"),
    })?;
    let content = Content::decode(&raw).map_err(|e| Error::ContentStream {
        page: page.index(),
        reason: format!("decode content: {e}"),
    })?;

    let mut has_text = false;
    let mut image_refs: Vec<Vec<u8>> = Vec::new();
    for op in &content.operations {
        match op.operator.as_str() {
            "BT" => has_text = true,
            "Tj" | "TJ" | "'" | "\"" => has_text = true,
            "Do" => {
                if let Some(Object::Name(name)) = op.operands.first() {
                    image_refs.push(name.clone());
                }
            }
            _ => {}
        }
    }
    if has_text {
        return Ok(false);
    }
    if image_refs.is_empty() {
        return Ok(false);
    }

    // Confirm at least one XObject reference resolves to /Subtype /Image.
    let resources = match doc.get_page_resources(page_id) {
        Ok((Some(r), _)) => r,
        _ => return Ok(false),
    };
    let xobjects = match resources.get(b"XObject").ok() {
        Some(Object::Dictionary(d)) => d.clone(),
        Some(Object::Reference(id)) => match doc.get_dictionary(*id) {
            Ok(d) => d.clone(),
            Err(_) => return Ok(false),
        },
        _ => return Ok(false),
    };
    for name in image_refs {
        let entry = xobjects.get(&name).ok();
        let stream = match entry {
            Some(Object::Reference(id)) => doc.get_object(*id).ok(),
            other => other,
        };
        if let Some(Object::Stream(s)) = stream {
            if let Ok(Object::Name(sub)) = s.dict.get(b"Subtype") {
                if sub == b"Image" {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
