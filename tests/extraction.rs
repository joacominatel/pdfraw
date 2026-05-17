//! Integration tests for end-to-end text extraction.

mod common;

use pdf_extractor::prelude::*;

#[test]
fn opens_pdf_and_reports_page_count() {
    let pdf = common::build_pdf(&[(20.0, 270.0, "Hello world")]);
    let doc = Document::from_bytes(pdf).unwrap();
    assert_eq!(doc.num_pages(), 1);
}

#[test]
fn extracts_chars_from_hello_world() {
    let pdf = common::build_pdf(&[(20.0, 270.0, "Hello")]);
    let doc = Document::from_bytes(pdf).unwrap();
    let page = doc.page(0).unwrap();
    let chars = page.chars().unwrap();
    let concatenated: String = chars.iter().map(|c| c.text.as_str()).collect();
    assert!(
        concatenated.contains("Hello"),
        "expected 'Hello' in extracted chars, got {concatenated:?}"
    );
}

#[test]
fn extracts_chars_have_positive_coordinates() {
    let pdf = common::build_pdf(&[(20.0, 270.0, "Hi")]);
    let doc = Document::from_bytes(pdf).unwrap();
    let page = doc.page(0).unwrap();
    let chars = page.chars().unwrap();
    assert!(!chars.is_empty(), "expected at least one char");
    for c in chars {
        assert!(c.x0 >= 0.0 && c.x1 > c.x0, "bad x: {:?}", c);
        assert!(c.top >= 0.0 && c.bottom > c.top, "bad y: {:?}", c);
    }
}

#[test]
fn extract_chars_for_multiple_lines_yields_distinct_tops() {
    let pdf = common::build_pdf(&[
        (20.0, 270.0, "first"),
        (20.0, 250.0, "second"),
    ]);
    let doc = Document::from_bytes(pdf).unwrap();
    let page = doc.page(0).unwrap();
    let chars = page.chars().unwrap();
    let mut tops: Vec<f32> = chars.iter().map(|c| c.top).collect();
    tops.sort_by(|a, b| a.partial_cmp(b).unwrap());
    tops.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    assert!(
        tops.len() >= 2,
        "expected two distinct line tops, got {tops:?}"
    );
}

#[test]
fn extract_text_returns_visible_string() {
    let pdf = common::build_pdf(&[(20.0, 270.0, "Hello world")]);
    let doc = Document::from_bytes(pdf).unwrap();
    let text = doc.page(0).unwrap().extract_text().unwrap();
    assert!(
        text.contains("Hello") && text.contains("world"),
        "got {text:?}"
    );
}

#[test]
fn extract_text_separates_lines_with_newline() {
    let pdf = common::build_pdf(&[
        (20.0, 270.0, "Above"),
        (20.0, 240.0, "Below"),
    ]);
    let doc = Document::from_bytes(pdf).unwrap();
    let text = doc.page(0).unwrap().extract_text().unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines.len() >= 2, "expected ≥2 lines, got {text:?}");
    assert!(text.contains("Above") && text.contains("Below"));
}

#[test]
fn words_have_increasing_x_within_line() {
    let pdf = common::build_pdf(&[(20.0, 270.0, "alpha beta gamma")]);
    let doc = Document::from_bytes(pdf).unwrap();
    let page = doc.page(0).unwrap();
    let words = page.words(&WordOptions::default()).unwrap();
    let mut last_x: f32 = f32::NEG_INFINITY;
    for w in &words {
        assert!(w.x0 >= last_x, "non-monotonic x: {:?}", w);
        last_x = w.x0;
    }
    let texts: Vec<_> = words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(texts, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn layout_preserves_invoice_columns() {
    // Mock a tiny invoice: emitter on the left, total on the right of the
    // same line, then items below.
    let pdf = common::build_pdf(&[
        (20.0, 270.0, "Acme Corp"),
        (150.0, 270.0, "Invoice #001"),
        (20.0, 230.0, "Item A"),
        (170.0, 230.0, "$10.00"),
        (20.0, 220.0, "Item B"),
        (170.0, 220.0, "$20.00"),
        (20.0, 200.0, "Total"),
        (170.0, 200.0, "$30.00"),
    ]);
    let doc = Document::from_bytes(pdf).unwrap();
    let page = doc.page(0).unwrap();
    let layout = page
        .extract_text_layout(&TextOptions::pdfplumber_defaults())
        .unwrap();

    // Right column words should follow their left column word on the same
    // line with multiple spaces.
    for (left, right) in [
        ("Acme", "Invoice"),
        ("Item A", "$10.00"),
        ("Item B", "$20.00"),
        ("Total", "$30.00"),
    ] {
        let line = layout
            .lines()
            .find(|l| l.contains(left) && l.contains(right))
            .unwrap_or_else(|| panic!("missing line with {left:?} and {right:?} in {layout:?}"));
        let l_idx = line.find(left).unwrap() + left.len();
        let r_idx = line.find(right).unwrap();
        let gap = r_idx - l_idx;
        assert!(
            gap >= 5,
            "expected ≥5 spaces between {left:?} and {right:?}, got {gap}: {line:?}"
        );
    }
}

#[test]
fn layout_snapshot_invoice() {
    let pdf = common::build_pdf(&[
        (20.0, 270.0, "ACME"),
        (150.0, 270.0, "Invoice"),
        (20.0, 240.0, "Line A"),
        (170.0, 240.0, "1.00"),
        (20.0, 225.0, "Line B"),
        (170.0, 225.0, "2.00"),
        (20.0, 200.0, "Total"),
        (170.0, 200.0, "3.00"),
    ]);
    let doc = Document::from_bytes(pdf).unwrap();
    let layout = doc
        .page(0)
        .unwrap()
        .extract_text_layout(&TextOptions::pdfplumber_defaults())
        .unwrap();
    insta::assert_snapshot!(layout);
}

#[test]
fn text_page_is_not_reported_as_scanned() {
    let pdf = common::build_pdf(&[(20.0, 270.0, "Real text")]);
    let doc = Document::from_bytes(pdf).unwrap();
    assert_eq!(doc.page(0).unwrap().is_scanned().unwrap(), false);
}

#[test]
fn page_metrics_match_a4() {
    let pdf = common::build_pdf(&[(20.0, 270.0, "x")]);
    let doc = Document::from_bytes(pdf).unwrap();
    let page = doc.page(0).unwrap();
    // A4 portrait in points: 210mm × 297mm ≈ 595.28 × 841.89
    assert!((page.width() - 595.28).abs() < 1.0, "width was {}", page.width());
    assert!((page.height() - 841.89).abs() < 1.0, "height was {}", page.height());
}
