//! Hand-built PDF fixtures for the adversarial test suite.
//!
//! `tests/common/mod.rs` can only produce well-formed Helvetica documents
//! through `printpdf`. The tests in this directory need hostile input:
//! unbalanced operators, degenerate page boxes, broken font dictionaries.
//! This module builds `lopdf::Document`s by hand so every byte of the page
//! dictionary and of the content stream is under test control.

#![allow(dead_code)]

use compact_str::CompactString;
use lopdf::{Dictionary, Document as LDoc, Object, ObjectId, Stream, dictionary};
use pdfraw::{Char, Document};

/// Builder for a single-page synthetic PDF.
pub struct PdfBuilder {
    doc: LDoc,
    fonts: Dictionary,
    page_entries: Dictionary,
    pages_entries: Dictionary,
    contents: Vec<Vec<u8>>,
    with_resources: bool,
}

impl Default for PdfBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfBuilder {
    /// A one-page document with a 612×792 `/MediaBox` and an empty content
    /// stream.
    pub fn new() -> Self {
        Self {
            doc: LDoc::with_version("1.7"),
            fonts: Dictionary::new(),
            page_entries: Dictionary::new(),
            pages_entries: Dictionary::new(),
            contents: Vec::new(),
            with_resources: true,
        }
    }

    /// Replace the raw (uncompressed) page content stream.
    pub fn content(mut self, raw: impl AsRef<[u8]>) -> Self {
        self.contents = vec![raw.as_ref().to_vec()];
        self
    }

    /// Split the page content across several `/Contents` streams, which the
    /// PDF spec allows at any lexical token boundary.
    pub fn contents(mut self, parts: &[&str]) -> Self {
        self.contents = parts.iter().map(|p| p.as_bytes().to_vec()).collect();
        self
    }

    /// Register a font dictionary under `/Resources /Font <name>`.
    pub fn font(mut self, name: &str, dict: Dictionary) -> Self {
        self.fonts
            .set(name.as_bytes().to_vec(), Object::Dictionary(dict));
        self
    }

    /// Register a font that lives behind an indirect reference.
    pub fn font_ref(mut self, name: &str, id: ObjectId) -> Self {
        self.fonts
            .set(name.as_bytes().to_vec(), Object::Reference(id));
        self
    }

    /// Drop the `/Resources` entry from the page dictionary entirely.
    pub fn without_resources(mut self) -> Self {
        self.with_resources = false;
        self
    }

    /// Add a free-standing indirect object and return its id.
    pub fn add_object(&mut self, obj: impl Into<Object>) -> ObjectId {
        self.doc.add_object(obj)
    }

    /// Set (or override) an entry of the page dictionary.
    pub fn page_entry(mut self, key: &str, value: impl Into<Object>) -> Self {
        self.page_entries.set(key.as_bytes().to_vec(), value.into());
        self
    }

    /// Set an entry on the parent `/Pages` node (to exercise inheritance).
    pub fn pages_entry(mut self, key: &str, value: impl Into<Object>) -> Self {
        self.pages_entries
            .set(key.as_bytes().to_vec(), value.into());
        self
    }

    /// Override `/MediaBox` on the page.
    pub fn media_box(self, value: impl Into<Object>) -> Self {
        self.page_entry("MediaBox", value)
    }

    /// Serialize to PDF bytes.
    pub fn into_bytes(mut self) -> Vec<u8> {
        let pages_id = self.doc.new_object_id();
        let parts = std::mem::take(&mut self.contents);
        let content_ids: Vec<Object> = if parts.is_empty() {
            vec![
                self.doc
                    .add_object(Stream::new(dictionary! {}, Vec::new()))
                    .into(),
            ]
        } else {
            parts
                .into_iter()
                .map(|p| self.doc.add_object(Stream::new(dictionary! {}, p)).into())
                .collect()
        };
        let contents: Object = if content_ids.len() == 1 {
            content_ids[0].clone()
        } else {
            Object::Array(content_ids)
        };

        let mut page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => contents,
        };
        if self.with_resources {
            page.set(
                "Resources",
                dictionary! { "Font" => Object::Dictionary(self.fonts.clone()) },
            );
        }
        if !self.page_entries.has(b"MediaBox") && !self.pages_entries.has(b"MediaBox") {
            page.set("MediaBox", vec![0.into(), 0.into(), 612.into(), 792.into()]);
        }
        for (k, v) in self.page_entries.iter() {
            page.set(k.clone(), v.clone());
        }
        let page_id = self.doc.add_object(page);

        let mut pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        for (k, v) in self.pages_entries.iter() {
            pages.set(k.clone(), v.clone());
        }
        self.doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog_id = self.doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        self.doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        self.doc.save_to(&mut buf).expect("serialize synthetic PDF");
        buf
    }

    /// Serialize and re-open through the public `pdfraw` entry point.
    pub fn build(self) -> Document {
        Document::from_bytes(self.into_bytes()).expect("synthetic PDF should load")
    }
}

/// Build a two-page document sharing one font, with the given raw content
/// streams and a `width` × `height` `/MediaBox` on both pages.
pub fn two_page_doc(page_a: &str, page_b: &str, width: i64, height: i64) -> Document {
    let mut doc = LDoc::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(Object::Dictionary(simple_font(
        "WinAnsiEncoding",
        65,
        &[1000, 1000],
    )));
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });

    let mut page_ids = Vec::new();
    for body in [page_a, page_b] {
        let content_id = doc.add_object(Stream::new(dictionary! {}, body.as_bytes().to_vec()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
        });
        page_ids.push(Object::from(page_id));
    }

    let count = page_ids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids,
            "Count" => count,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("serialize synthetic PDF");
    Document::from_bytes(buf).expect("synthetic PDF should load")
}

/// A minimal simple (non-composite) font dictionary.
///
/// `widths` are raw PDF glyph-space units (1/1000 em) starting at
/// `first_char`.
pub fn simple_font(base_encoding: &str, first_char: i64, widths: &[i64]) -> Dictionary {
    let mut d = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "FirstChar" => first_char,
        "Widths" => widths.iter().map(|w| Object::Integer(*w)).collect::<Vec<_>>(),
    };
    if !base_encoding.is_empty() {
        d.set("Encoding", Object::Name(base_encoding.as_bytes().to_vec()));
    }
    d
}

/// Collect the chars of page `0`, owning the result so callers do not have
/// to keep the temporary `Page` handle alive.
pub fn page_chars(doc: &Document) -> Vec<Char> {
    let page = doc.page(0).expect("page 0");
    page.chars().expect("chars").to_vec()
}

/// Build a `Char` with sane defaults for the geometry-free layout tests.
pub fn ch(text: &str, x0: f32, x1: f32, top: f32, bottom: f32) -> Char {
    Char {
        text: CompactString::from(text),
        x0,
        x1,
        top,
        bottom,
        doctop: top,
        size: 12.0,
        fontname: CompactString::from("F1"),
        upright: true,
    }
}

/// Run `f` on a worker thread and fail the test if it does not finish
/// within `secs`. Used to probe for unbounded loops without hanging CI.
pub fn with_timeout<T: Send + 'static>(
    secs: u64,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(std::time::Duration::from_secs(secs))
        .map_err(|_| format!("work did not finish within {secs}s"))
}
