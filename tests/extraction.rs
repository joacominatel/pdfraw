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
fn page_metrics_match_a4() {
    let pdf = common::build_pdf(&[(20.0, 270.0, "x")]);
    let doc = Document::from_bytes(pdf).unwrap();
    let page = doc.page(0).unwrap();
    // A4 portrait in points: 210mm × 297mm ≈ 595.28 × 841.89
    assert!((page.width() - 595.28).abs() < 1.0, "width was {}", page.width());
    assert!((page.height() - 841.89).abs() < 1.0, "height was {}", page.height());
}
