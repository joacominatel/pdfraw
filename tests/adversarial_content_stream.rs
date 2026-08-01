//! Adversarial tests for the content-stream state machine.
//!
//! Each test feeds `pdfraw` a hand-written operator sequence that is legal
//! (or at least survivable) PDF but unusual, and checks the resulting
//! glyph geometry. Tests marked `#[ignore = "BUG: ..."]` reproduce a real
//! defect.

mod synthetic;

use lopdf::{Object, dictionary};
use pdfraw::prelude::*;
use synthetic::{PdfBuilder, page_chars, simple_font, with_timeout};

/// A 1000/1000-em font covering `A` and `B`.
fn ab_font() -> lopdf::Dictionary {
    simple_font("WinAnsiEncoding", 65, &[1000, 1000])
}

fn chars_of(content: &str) -> Vec<Char> {
    let doc = PdfBuilder::new()
        .font("F1", ab_font())
        .content(content)
        .build();
    page_chars(&doc)
}

// ---------------------------------------------------------------------------
// Text-object framing
// ---------------------------------------------------------------------------

#[test]
fn text_shown_without_bt_is_still_extracted() {
    let chars = chars_of("/F1 10 Tf 100 700 Td <41> Tj");
    assert_eq!(
        chars.len(),
        1,
        "text drawn outside a text object was dropped: {chars:?}"
    );
}

#[test]
fn text_shown_after_stray_et_is_still_extracted() {
    let chars = chars_of("BT /F1 10 Tf 100 700 Td ET <41> Tj");
    assert_eq!(chars.len(), 1, "text after a stray ET was dropped");
}

#[test]
fn nested_bt_resets_the_text_matrix() {
    // Inner BT must reset Tm, so the second glyph starts at the Td of the
    // inner text object, not at the outer one.
    let chars = chars_of("BT /F1 10 Tf 300 700 Td BT /F1 10 Tf 100 700 Td <41> Tj ET ET");
    assert_eq!(chars.len(), 1);
    assert!(
        (chars[0].x0 - 100.0).abs() < 0.5,
        "inner BT did not reset the text matrix: x0={}",
        chars[0].x0
    );
}

#[test]
fn unbalanced_q_does_not_lose_text() {
    let chars = chars_of("Q Q Q BT /F1 10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1, "text lost after extra Q operators");
}

#[test]
fn cm_with_too_few_operands_is_ignored() {
    let chars = chars_of("1 0 0 1 100 cm BT /F1 10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
    assert!(
        (chars[0].x0 - 100.0).abs() < 0.5,
        "malformed cm changed the CTM: x0={}",
        chars[0].x0
    );
}

// ---------------------------------------------------------------------------
// Text state
// ---------------------------------------------------------------------------

#[test]
#[ignore = "BUG: horizontal scaling (Tz) is applied to the glyph advance but not to the text rendering matrix, so reported glyph widths ignore Tz"]
fn horizontal_scaling_widens_the_reported_glyph_box() {
    let scaled = chars_of("BT /F1 10 Tf 200 Tz 100 700 Td <4142> Tj ET");
    assert_eq!(scaled.len(), 2);
    let advance = scaled[1].x0 - scaled[0].x0;
    let width = scaled[0].x1 - scaled[0].x0;
    assert!(
        (advance - 20.0).abs() < 0.5,
        "Tz 200 should double the advance, got {advance}"
    );
    assert!(
        (width - 20.0).abs() < 0.5,
        "Tz 200 should double the glyph box, got {width} for an advance of {advance}"
    );
}

#[test]
fn negative_font_size_keeps_the_char_box_ordered() {
    let chars = chars_of("BT /F1 -10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
    let c = &chars[0];
    assert!(c.x1 >= c.x0, "x1 < x0: {c:?}");
    assert!(c.bottom >= c.top, "bottom < top: {c:?}");
}

#[test]
fn char_size_reports_the_effective_font_size() {
    let chars = chars_of("BT /F1 24 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
    assert!(
        (chars[0].size - 24.0).abs() < 0.5,
        "24pt text reported size={} (glyph box height is {})",
        chars[0].size,
        chars[0].bottom - chars[0].top
    );
}

#[test]
fn char_size_combines_the_font_size_and_the_ctm_scale() {
    let chars = chars_of("2 0 0 2 0 0 cm BT /F1 10 Tf 100 300 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
    assert!(
        (chars[0].size - 20.0).abs() < 0.5,
        "10pt text under a 2x CTM reported size={}",
        chars[0].size
    );
}

#[test]
fn zero_font_size_does_not_panic() {
    let chars = chars_of("BT /F1 0 Tf 100 700 Td <4142> Tj ET");
    assert_eq!(chars.len(), 2, "glyphs dropped at font size 0");
}

#[test]
fn zero_horizontal_scaling_does_not_panic() {
    let chars = chars_of("BT /F1 10 Tf 0 Tz 100 700 Td <4142> Tj ET");
    assert_eq!(chars.len(), 2, "glyphs dropped at Tz 0");
}

#[test]
fn negative_horizontal_scaling_does_not_panic() {
    let chars = chars_of("BT /F1 10 Tf -100 Tz 100 700 Td <4142> Tj ET");
    assert_eq!(chars.len(), 2, "glyphs dropped at negative Tz");
}

#[test]
fn tf_naming_a_font_absent_from_resources_still_emits_text() {
    let chars = chars_of("BT /NoSuchFont 10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1, "unknown font dropped the glyph");
    assert_eq!(chars[0].text.as_str(), "A");
}

#[test]
#[ignore = "BUG: word spacing is keyed on the decoded character instead of on byte code 32, so Tw is skipped when /Differences remaps code 32 (PDF 32000-1 §9.3.3)"]
fn word_spacing_applies_to_byte_code_32_even_when_remapped() {
    let mut widths = vec![1000_i64; 34]; // codes 32..=65
    widths[0] = 1000;
    let mut font = simple_font("", 32, &widths);
    font.set(
        "Encoding",
        dictionary! {
            "Differences" => vec![
                Object::Integer(32),
                Object::Name(b"bullet".to_vec()),
            ],
        },
    );
    let doc = PdfBuilder::new()
        .font("F1", font)
        .content("BT /F1 10 Tf 100 Tw 100 700 Td <2041> Tj ET")
        .build();
    let chars = page_chars(&doc);
    assert_eq!(chars.len(), 2, "expected two glyphs, got {chars:?}");
    assert_eq!(chars[1].text.as_str(), "A");
    assert!(
        (chars[1].x0 - 210.0).abs() < 0.5,
        "Tw was not applied to byte code 32: second glyph at x0={}",
        chars[1].x0
    );
}

// ---------------------------------------------------------------------------
// TJ arrays
// ---------------------------------------------------------------------------

#[test]
fn tj_with_nested_arrays_and_nulls_does_not_panic() {
    let chars = chars_of("BT /F1 10 Tf 100 700 Td [<41> [<42>] null /Name <42>] TJ ET");
    let text: String = chars.iter().map(|c| c.text.as_str()).collect();
    assert!(
        text.contains('A'),
        "well-formed items of a hostile TJ array were dropped: {text:?}"
    );
}

#[test]
fn huge_tj_adjustment_does_not_inflate_layout_output() {
    let doc = PdfBuilder::new()
        .font("F1", ab_font())
        .content("BT /F1 10 Tf 100 700 Td [<41> -2000000000 <42>] TJ ET")
        .build();
    let out = with_timeout(20, move || {
        doc.page(0)
            .unwrap()
            .extract_text_layout(&TextOptions::pdfplumber_defaults())
            .unwrap()
    })
    .expect("layout extraction must terminate");
    assert!(
        out.len() < 100_000,
        "a two-glyph page produced {} output chars",
        out.len()
    );
}

#[test]
fn tj_with_i64_min_adjustment_does_not_panic() {
    let chars = chars_of("BT /F1 10 Tf 100 700 Td [<41> -9223372036854775808 <42>] TJ ET");
    assert_eq!(chars.len(), 2, "glyphs lost on an i64::MIN adjustment");
    assert!(
        chars.iter().all(|c| !c.x0.is_nan()),
        "an i64::MIN adjustment produced NaN coordinates: {chars:?}"
    );
}

// ---------------------------------------------------------------------------
// Positioning matrices
// ---------------------------------------------------------------------------

#[test]
fn singular_text_matrix_does_not_panic() {
    let chars = chars_of("BT /F1 10 Tf 0 0 0 0 0 0 Tm <41> Tj ET");
    assert!(
        chars.len() <= 1,
        "singular Tm produced unexpected glyphs: {chars:?}"
    );
}

#[test]
fn rotated_text_matrix_marks_chars_as_not_upright() {
    // 90° rotation: [0 1 -1 0 100 700] Tm
    let chars = chars_of("BT /F1 10 Tf 0 1 -1 0 100 700 Tm <41> Tj ET");
    assert_eq!(chars.len(), 1);
    assert!(!chars[0].upright, "rotated glyph reported as upright");
}

#[test]
fn quote_operator_with_too_few_operands_does_not_panic() {
    let chars = chars_of("BT /F1 10 Tf 100 700 Td 5 <41> \" ET");
    assert!(
        chars.len() <= 1,
        "malformed \" produced unexpected glyphs: {chars:?}"
    );
}

// ---------------------------------------------------------------------------
// Content stream splitting
// ---------------------------------------------------------------------------

#[test]
fn text_survives_a_split_across_two_content_streams() {
    let doc = PdfBuilder::new()
        .font("F1", ab_font())
        .contents(&["BT /F1 10 Tf 100 700 Td <41> Tj", "ET"])
        .build();
    let chars = page_chars(&doc);
    assert_eq!(
        chars.len(),
        1,
        "text was lost across the /Contents boundary: {chars:?}"
    );
}

// ---------------------------------------------------------------------------
// Volume
// ---------------------------------------------------------------------------

#[test]
fn deeply_nested_q_operators_do_not_overflow() {
    let mut content = String::new();
    for _ in 0..50_000 {
        content.push_str("q ");
    }
    content.push_str("BT /F1 10 Tf 100 700 Td <41> Tj ET");
    let chars = with_timeout(30, move || chars_of(&content)).expect("must terminate");
    assert_eq!(chars.len(), 1);
}
