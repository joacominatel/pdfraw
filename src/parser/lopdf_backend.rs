//! `lopdf`-based PDF parser.
//!
//! This module is the bridge between the byte-level PDF representation
//! parsed by `lopdf` and the positioned [`Char`] output consumed by the
//! text layer. Implementation arrives in phase 1.

use crate::char::Char;
use crate::error::{Error, Result};
use crate::page::{Page, PageMetrics};
use lopdf::{Dictionary, Document as LDoc, Object, ObjectId};

/// Read width/height/rotation for the page.
pub(crate) fn page_metrics(doc: &LDoc, page_id: ObjectId) -> Result<PageMetrics> {
    let page = doc.get_dictionary(page_id).map_err(Error::from)?;
    let media_box = resolve_inheritable(doc, page, b"MediaBox")
        .ok_or_else(|| Error::ContentStream {
            page: 0,
            reason: "missing /MediaBox".into(),
        })?;
    let (width, height) = media_box_dimensions(&media_box)?;
    let rotation = resolve_inheritable(doc, page, b"Rotate")
        .and_then(|o| match o {
            Object::Integer(i) => Some(i as i16),
            _ => None,
        })
        .unwrap_or(0);
    Ok(PageMetrics { width, height, rotation })
}

fn resolve_inheritable(doc: &LDoc, page: &Dictionary, key: &[u8]) -> Option<Object> {
    if let Ok(v) = page.get(key) {
        return Some(deref(doc, v.clone()));
    }
    let mut cursor = page.clone();
    while let Ok(parent) = cursor.get(b"Parent") {
        if let Object::Reference(id) = parent {
            if let Ok(d) = doc.get_dictionary(*id) {
                if let Ok(v) = d.get(key) {
                    return Some(deref(doc, v.clone()));
                }
                cursor = d.clone();
                continue;
            }
        }
        break;
    }
    None
}

fn deref(doc: &LDoc, obj: Object) -> Object {
    match obj {
        Object::Reference(id) => doc.get_object(id).cloned().unwrap_or(Object::Null),
        other => other,
    }
}

fn media_box_dimensions(o: &Object) -> Result<(f32, f32)> {
    let arr = match o {
        Object::Array(a) => a,
        _ => {
            return Err(Error::ContentStream {
                page: 0,
                reason: "/MediaBox is not an array".into(),
            });
        }
    };
    if arr.len() != 4 {
        return Err(Error::ContentStream {
            page: 0,
            reason: format!("/MediaBox has {} elements, expected 4", arr.len()),
        });
    }
    let n = |o: &Object| -> Result<f32> {
        match o {
            Object::Integer(i) => Ok(*i as f32),
            Object::Real(r) => Ok(*r),
            _ => Err(Error::ContentStream {
                page: 0,
                reason: "/MediaBox contains a non-numeric value".into(),
            }),
        }
    };
    let x0 = n(&arr[0])?;
    let y0 = n(&arr[1])?;
    let x1 = n(&arr[2])?;
    let y1 = n(&arr[3])?;
    Ok(((x1 - x0).abs(), (y1 - y0).abs()))
}

/// Extract every glyph on the page as a [`Char`].
///
/// Implementation lands in phase 1. Stub returns an empty vec.
pub(crate) fn extract_chars(_page: &Page<'_>) -> Result<Vec<Char>> {
    Ok(Vec::new())
}
