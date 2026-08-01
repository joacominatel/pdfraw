//! Heuristic detection of pages whose text layer is empty.
//!
//! A page is reported as "scanned" when:
//! 1. its content stream emits **no text-show operators** (no `BT`, `Tj`,
//!    `TJ`, `'`, or `"`), and
//! 2. it draws **some image-like content**: at least one `Do` invocation
//!    (any XObject — `/Image` or `/Form`) or at least one inline image
//!    (`BI`).
//!
//! We deliberately do not descend into the XObject dictionary to verify
//! `Subtype = /Image`. PDFs in the wild often wrap a scan inside a Form
//! XObject whose subtype the simple resource lookup cannot reach (e.g.
//! when resources live on an ancestor in the Pages tree). Since the
//! crate's state machine does not descend into Form XObjects either, a
//! page that draws nothing but `Do /Form` produces no extractable text
//! in practice — the caller benefits from knowing.
//!
//! Pages that draw nothing at all (no text, no images, only paths or
//! totally empty) return `false`. That matches the public contract:
//! `is_scanned == true` means "OCR is probably what you want".

use crate::error::{Error, Result};
use crate::page::Page;
use lopdf::content::Content;

/// Returns `true` when the page appears to need OCR — no text operators
/// were emitted but image-bearing operators were.
pub(crate) fn is_scanned(page: &Page<'_>) -> Result<bool> {
    let doc = &page.document().inner;
    let page_id = page.page_id();
    let raw = doc.get_page_content(page_id);
    let content = Content::decode(&raw).map_err(|e| Error::ContentStream {
        page: page.index(),
        reason: format!("decode content: {e}"),
    })?;

    let mut has_text = false;
    let mut has_image_like = false;
    for op in &content.operations {
        match op.operator.as_str() {
            "BT" | "Tj" | "TJ" | "'" | "\"" => has_text = true,
            "Do" | "BI" => has_image_like = true,
            _ => {}
        }
        // Early-exit: once we know there is text, the answer is fixed.
        if has_text {
            return Ok(false);
        }
    }
    Ok(has_image_like)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use lopdf::content::Operation;
    use lopdf::{Object, Stream, dictionary};

    /// Build a single-page `lopdf::Document` whose page content is the
    /// given list of operators. Resources are intentionally minimal — the
    /// heuristic should not need to resolve any dictionary.
    fn doc_with_ops(ops: Vec<Operation>) -> Document {
        let mut doc = lopdf::Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let content = Content { operations: ops };
        let content_stream = Stream::new(dictionary! {}, content.encode().unwrap());
        let content_id = doc.add_object(content_stream);
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        // Convert the in-memory lopdf::Document into our public Document.
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        Document::from_bytes(buf).unwrap()
    }

    fn op(name: &str, operands: Vec<Object>) -> Operation {
        Operation::new(name, operands)
    }

    #[test]
    fn returns_false_when_page_has_text() {
        let doc = doc_with_ops(vec![
            op("BT", vec![]),
            op("Tj", vec![Object::string_literal("hi")]),
            op("ET", vec![]),
        ]);
        assert!(!doc.page(0).unwrap().is_scanned().unwrap());
    }

    #[test]
    fn returns_true_when_page_has_only_do_invocation() {
        // Mimics Comprobante-style PDFs: an Xobject invocation with no
        // text operators. We do not need a real XObject in resources —
        // the heuristic only inspects the operator stream.
        let doc = doc_with_ops(vec![
            op("q", vec![]),
            op(
                "cm",
                vec![1.into(), 0.into(), 0.into(), 1.into(), 0.into(), 0.into()],
            ),
            op("Do", vec![Object::Name(b"Im1".to_vec())]),
            op("Q", vec![]),
        ]);
        assert!(doc.page(0).unwrap().is_scanned().unwrap());
    }

    // Inline images (`BI ... ID ... EI`) use a non-standard token shape
    // that `Content::encode` does not round-trip cleanly, so we don't
    // synthesise a fixture for that case here. The branch is exercised
    // implicitly: the heuristic checks `op.operator == "BI"` on the
    // decoded operation list, which lopdf populates straightforwardly
    // from real PDFs that contain `BI/ID/EI`.

    #[test]
    fn returns_false_when_page_is_empty() {
        let doc = doc_with_ops(vec![]);
        assert!(!doc.page(0).unwrap().is_scanned().unwrap());
    }

    #[test]
    fn returns_false_when_page_has_only_path_operators() {
        let doc = doc_with_ops(vec![
            op("m", vec![0.into(), 0.into()]),
            op("l", vec![10.into(), 10.into()]),
            op("S", vec![]),
        ]);
        assert!(!doc.page(0).unwrap().is_scanned().unwrap());
    }
}
