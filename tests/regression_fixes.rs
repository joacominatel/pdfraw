//! Regression tests for defects found during the v0.1 audit.
//!
//! Every test here reproduces a concrete bug through the public API. They
//! are written before the fix, so on a pre-fix checkout they fail (or, for
//! the hang, time out).

mod common;

use compact_str::CompactString;
use lopdf::content::{Content, Operation};
use lopdf::{Object, Stream, dictionary};
use pdfraw::prelude::*;
use pdfraw::{Char, WordOptions};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Run `f` on a worker thread and fail the test if it does not finish in
/// time. Used to turn a potential infinite loop into a test failure rather
/// than a suite that hangs forever.
fn with_timeout<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(v) => v,
        Err(_) => panic!("operation did not terminate within {secs}s — probable infinite loop"),
    }
}

fn op(name: &str, operands: Vec<Object>) -> Operation {
    Operation::new(name, operands)
}

fn num(v: f32) -> Object {
    Object::Real(v)
}

// ============================================================================
// A cyclic /Parent chain must not hang the page-metrics walk
// ============================================================================

#[test]
fn page_metrics_terminate_when_parent_chain_is_cyclic() {
    // Two /Pages nodes pointing at each other, and no /MediaBox anywhere in
    // the chain, so the inheritable lookup never finds its key and keeps
    // walking. A malformed (or hostile) PDF can be shaped exactly like this.
    let bytes = with_timeout(10, || {
        let mut doc = lopdf::Document::with_version("1.7");
        let node_a = doc.new_object_id();
        let node_b = doc.new_object_id();

        let content = Content {
            operations: vec![op("BT", vec![]), op("ET", vec![])],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => node_a,
            "Contents" => content_id,
        });
        doc.objects.insert(
            node_a,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Parent" => node_b,
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        // The cycle: B's parent is A again.
        doc.objects.insert(
            node_b,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Parent" => node_a,
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => node_a,
        });
        doc.trailer.set("Root", catalog_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    });

    let width = with_timeout(10, move || {
        let doc = Document::from_bytes(bytes).unwrap();
        let page = doc.page(0).unwrap();
        page.width()
    });

    // The chain has no /MediaBox, so the fallback A4-ish default applies.
    // The point of the test is that we get *an* answer at all.
    assert!(width > 0.0, "expected a fallback width, got {width}");
}

// ============================================================================
// q/Q must save and restore the text state, not just the CTM
// ============================================================================

/// Build a single-page document whose content stream is `ops`, with two
/// Helvetica fonts registered as /F1 and /F2 in the page resources.
fn doc_with_ops(ops: Vec<Operation>) -> Document {
    let mut doc = lopdf::Document::with_version("1.7");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });

    let content = Content { operations: ops };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => dictionary! {
            "Font" => dictionary! { "F1" => font_id, "F2" => font_id },
        },
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

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    Document::from_bytes(buf).unwrap()
}

#[test]
fn restore_operator_reverts_font_size_set_inside_saved_state() {
    // PDF 32000-1 Table 52: the font and size set by Tf belong to the
    // graphics state, so `Q` must restore whatever was in effect at `q`.
    let doc = doc_with_ops(vec![
        op("BT", vec![]),
        op("Tf", vec![Object::Name(b"F1".to_vec()), num(10.0)]),
        op("Td", vec![num(50.0), num(700.0)]),
        op("Tj", vec![Object::string_literal("A")]),
        op("q", vec![]),
        op("Tf", vec![Object::Name(b"F2".to_vec()), num(40.0)]),
        op("Td", vec![num(0.0), num(-20.0)]),
        op("Tj", vec![Object::string_literal("B")]),
        op("Q", vec![]),
        op("Td", vec![num(0.0), num(-20.0)]),
        op("Tj", vec![Object::string_literal("C")]),
        op("ET", vec![]),
    ]);

    let page = doc.page(0).unwrap();
    let chars = page.chars().unwrap();
    let find = |t: &str| {
        chars
            .iter()
            .find(|c| c.text.as_str() == t)
            .unwrap_or_else(|| panic!("no char {t:?} in {chars:?}"))
    };

    let a = find("A");
    let b = find("B");
    let c = find("C");

    assert!(
        (b.size - 40.0).abs() < 0.5,
        "B was drawn inside q/Q at size 40, got {}",
        b.size
    );
    assert!(
        (c.size - a.size).abs() < 0.5,
        "Q must restore the size in effect at q: A={}, C={}",
        a.size,
        c.size
    );
}

#[test]
fn restore_operator_reverts_char_spacing_set_inside_saved_state() {
    // Same rule for Tc: set inside q/Q, it must not leak past the Q.
    let doc = doc_with_ops(vec![
        op("BT", vec![]),
        op("Tf", vec![Object::Name(b"F1".to_vec()), num(10.0)]),
        op("Td", vec![num(50.0), num(700.0)]),
        op("Tj", vec![Object::string_literal("XY")]),
        op("q", vec![]),
        op("Tc", vec![num(30.0)]),
        op("Q", vec![]),
        op("Td", vec![num(0.0), num(-30.0)]),
        op("Tj", vec![Object::string_literal("XY")]),
        op("ET", vec![]),
    ]);

    let page = doc.page(0).unwrap();
    let chars = page.chars().unwrap();
    let xs: Vec<f32> = chars.iter().map(|c| c.x0).collect();
    assert_eq!(chars.len(), 4, "expected 4 glyphs, got {chars:?}");

    let first_advance = xs[1] - xs[0];
    let second_advance = xs[3] - xs[2];
    assert!(
        (first_advance - second_advance).abs() < 0.5,
        "Tc set inside q/Q leaked past the Q: advances {first_advance} vs {second_advance}"
    );
}

// ============================================================================
// Char::size must be the effective font size, not the matrix scale
// ============================================================================

#[test]
fn char_size_reports_the_font_size_set_by_tf() {
    // `Tf /F1 12` under an identity text matrix must yield size 12, not the
    // bare scale factor of Tm x CTM.
    let doc = doc_with_ops(vec![
        op("BT", vec![]),
        op("Tf", vec![Object::Name(b"F1".to_vec()), num(12.0)]),
        op("Td", vec![num(50.0), num(700.0)]),
        op("Tj", vec![Object::string_literal("A")]),
        op("ET", vec![]),
    ]);

    let page = doc.page(0).unwrap();
    let chars = page.chars().unwrap();
    assert_eq!(chars.len(), 1);
    assert!(
        (chars[0].size - 12.0).abs() < 0.01,
        "expected size 12.0, got {}",
        chars[0].size
    );
}

#[test]
fn char_size_scales_with_the_text_matrix() {
    // A Tm that doubles the scale doubles the effective size on the page.
    let doc = doc_with_ops(vec![
        op("BT", vec![]),
        op("Tf", vec![Object::Name(b"F1".to_vec()), num(10.0)]),
        op(
            "Tm",
            vec![
                num(2.0),
                num(0.0),
                num(0.0),
                num(2.0),
                num(50.0),
                num(700.0),
            ],
        ),
        op("Tj", vec![Object::string_literal("A")]),
        op("ET", vec![]),
    ]);

    let page = doc.page(0).unwrap();
    let chars = page.chars().unwrap();
    assert!(
        (chars[0].size - 20.0).abs() < 0.01,
        "expected size 20.0 (10pt font under a 2x matrix), got {}",
        chars[0].size
    );
}

// ============================================================================
// Char::doctop must accumulate the height of preceding pages
// ============================================================================

#[test]
fn doctop_offsets_by_the_height_of_preceding_pages() {
    let bytes = common::build_two_page_pdf(&[(20.0, 250.0, "First")], &[(20.0, 250.0, "Second")]);
    let doc = Document::from_bytes(bytes).unwrap();

    let p0 = doc.page(0).unwrap();
    let p1 = doc.page(1).unwrap();
    let page_height = p0.height();

    let c0 = p0.chars().unwrap()[0].clone();
    let c1 = p1.chars().unwrap()[0].clone();

    assert!(
        (c0.doctop - c0.top).abs() < 0.01,
        "on the first page doctop equals top"
    );
    assert!(
        (c1.doctop - (c1.top + page_height)).abs() < 0.5,
        "page 1 doctop must be top + {page_height}: top={}, doctop={}",
        c1.top,
        c1.doctop
    );
}

// ============================================================================
// Word::char_range must cover the word's source chars
// ============================================================================

fn mk(text: &str, x0: f32, x1: f32, top: f32) -> Char {
    Char {
        text: CompactString::from(text),
        x0,
        x1,
        top,
        bottom: top + 12.0,
        doctop: top,
        size: 12.0,
        fontname: CompactString::from("F1"),
        upright: true,
    }
}

#[test]
fn char_range_covers_source_chars_when_stream_order_is_reversed() {
    // A content stream may emit glyphs right-to-left even though they read
    // left-to-right on the page. After the positional sort, the word's first
    // char sits at a *higher* index than its last char.
    let chars = vec![
        mk("i", 5.0, 8.0, 10.0), // index 0, but drawn second on the page
        mk("H", 0.0, 5.0, 10.0), // index 1, but drawn first on the page
    ];

    let words = extract_words_public(&chars);
    assert_eq!(words.len(), 1, "expected one word, got {words:?}");
    let w = &words[0];
    assert_eq!(w.text.as_str(), "Hi");

    let range = w.char_range.clone();
    assert!(
        !range.is_empty(),
        "char_range must not be empty for a 2-char word, got {range:?}"
    );
    assert_eq!(
        range.len(),
        2,
        "char_range must cover both source chars, got {range:?}"
    );
    assert!(
        range.end <= chars.len(),
        "char_range must stay in bounds of the char slice, got {range:?}"
    );
}

fn extract_words_public(chars: &[Char]) -> Vec<pdfraw::Word> {
    pdfraw::text::extractor::extract_words(chars, &WordOptions::default())
}
