//! Duplicate-glyph removal.
//!
//! A PDF with no bold cut for its font often fakes one by drawing the same
//! string twice, a fraction of a point apart. The glyphs are genuinely in the
//! file twice, so the parser is right to report both — but the layout grid
//! then interleaves them and `SALDO ANTERIOR` comes out as
//! `SSAALLDDOO AANNTTEERRIIOORR`.
//!
//! These tests pin the filter that removes the second copy, and — just as
//! importantly — pin what it must *not* remove.

mod synthetic;

use pdfraw::text::dedupe::dedupe_chars;
use synthetic::ch;

/// `ch` with the font, size and uprightness spelled out, since those take
/// part in the identity of a glyph.
fn styled(text: &str, x0: f32, top: f32, font: &str, size: f32) -> pdfraw::Char {
    let mut c = ch(text, x0, x0 + 5.0, top, top + 10.0);
    c.fontname = font.into();
    c.size = size;
    c.doctop = top;
    c
}

fn plain(text: &str, x0: f32, top: f32) -> pdfraw::Char {
    styled(text, x0, top, "F1", 10.0)
}

fn texts(chars: &[pdfraw::Char]) -> String {
    chars.iter().map(|c| c.text.as_str()).collect()
}

#[test]
fn a_double_struck_glyph_collapses_to_one() {
    // The real offset from an ICBC statement.
    let chars = vec![plain("S", 150.0, 305.0), plain("S", 150.3, 305.0)];
    assert_eq!(texts(&dedupe_chars(&chars, 1.0)), "S");
}

#[test]
fn a_double_struck_word_collapses_to_one() {
    let mut chars = Vec::new();
    for (i, t) in ["S", "A", "L", "D", "O"].iter().enumerate() {
        chars.push(plain(t, 150.0 + i as f32 * 4.2, 305.0));
    }
    for (i, t) in ["S", "A", "L", "D", "O"].iter().enumerate() {
        chars.push(plain(t, 150.3 + i as f32 * 4.2, 305.0));
    }
    assert_eq!(texts(&dedupe_chars(&chars, 1.0)), "SALDO");
}

#[test]
fn distinct_glyphs_at_the_same_spot_both_survive() {
    // Different text is never a duplicate, however much the boxes overlap.
    let chars = vec![plain("A", 100.0, 200.0), plain("B", 100.0, 200.0)];
    assert_eq!(dedupe_chars(&chars, 1.0).len(), 2);
}

#[test]
fn a_repeated_letter_further_apart_than_tolerance_survives() {
    // "AA" set normally: two real glyphs, one advance apart.
    let chars = vec![plain("A", 100.0, 200.0), plain("A", 105.0, 200.0)];
    assert_eq!(
        texts(&dedupe_chars(&chars, 1.0)),
        "AA",
        "an ordinary doubled letter is not a double strike"
    );
}

#[test]
fn the_same_letter_on_a_different_line_survives() {
    let chars = vec![plain("A", 100.0, 200.0), plain("A", 100.0, 220.0)];
    assert_eq!(dedupe_chars(&chars, 1.0).len(), 2);
}

#[test]
fn a_different_font_is_not_a_duplicate() {
    let chars = vec![
        styled("A", 100.0, 200.0, "F1", 10.0),
        styled("A", 100.2, 200.0, "F2", 10.0),
    ];
    assert_eq!(dedupe_chars(&chars, 1.0).len(), 2);
}

#[test]
fn a_different_size_is_not_a_duplicate() {
    let chars = vec![
        styled("A", 100.0, 200.0, "F1", 10.0),
        styled("A", 100.2, 200.0, "F1", 12.0),
    ];
    assert_eq!(dedupe_chars(&chars, 1.0).len(), 2);
}

#[test]
fn survivors_keep_their_original_order() {
    let chars = vec![
        plain("H", 100.0, 200.0),
        plain("I", 104.0, 200.0),
        plain("H", 100.3, 200.0),
        plain("I", 104.3, 200.0),
    ];
    assert_eq!(texts(&dedupe_chars(&chars, 1.0)), "HI");
}

#[test]
fn the_earliest_copy_is_the_one_kept() {
    // Ties break on (doctop, x0), so the survivor is the leftmost/highest —
    // not merely the first one the stream happened to draw.
    let chars = vec![plain("S", 150.3, 305.0), plain("S", 150.0, 305.0)];
    let kept = dedupe_chars(&chars, 1.0);
    assert_eq!(kept.len(), 1);
    assert!(
        (kept[0].x0 - 150.0).abs() < 0.001,
        "kept x0 = {}",
        kept[0].x0
    );
}

#[test]
fn zero_tolerance_removes_nothing_but_exact_repeats() {
    let chars = vec![plain("S", 150.0, 305.0), plain("S", 150.3, 305.0)];
    assert_eq!(
        dedupe_chars(&chars, 0.0).len(),
        2,
        "a tolerance of 0 must not merge glyphs that differ in position"
    );
}

#[test]
fn an_empty_slice_is_handled() {
    assert!(dedupe_chars(&[], 1.0).is_empty());
}

#[test]
fn a_non_finite_position_does_not_panic() {
    let chars = vec![
        plain("A", f32::NAN, 200.0),
        plain("A", 100.0, f32::INFINITY),
        plain("A", 100.0, 200.0),
    ];
    let kept = dedupe_chars(&chars, 1.0);
    assert!(!kept.is_empty(), "everything was discarded");
}

#[test]
fn clustering_chains_across_the_tolerance() {
    // Three copies each 0.6 apart: no two adjacent gaps exceed 1.0, so all
    // three belong to one cluster even though the ends are 1.2 apart. This
    // is the same chaining rule the line grouping already uses.
    let chars = vec![
        plain("S", 150.0, 305.0),
        plain("S", 150.6, 305.0),
        plain("S", 151.2, 305.0),
    ];
    assert_eq!(texts(&dedupe_chars(&chars, 1.0)), "S");
}

// ---------------------------------------------------------------------------
// Wiring into the public API
// ---------------------------------------------------------------------------

#[test]
fn layout_leaves_double_strikes_alone_by_default() {
    let doc = double_struck_page();
    let page = doc.page(0).unwrap();
    let text = page
        .extract_text_layout(&pdfraw::TextOptions::pdfplumber_defaults())
        .unwrap();
    assert!(
        text.contains("HHII"),
        "the default must report the file as it is: {text:?}"
    );
}

#[test]
fn layout_collapses_double_strikes_when_asked() {
    let doc = double_struck_page();
    let page = doc.page(0).unwrap();
    let opts = pdfraw::TextOptions::builder().dedupe_tolerance(1.0).build();
    let text = page.extract_text_layout(&opts).unwrap();
    assert!(text.contains("HI"), "still doubled: {text:?}");
    assert!(!text.contains("HHII"), "still doubled: {text:?}");
}

#[test]
fn page_dedupe_chars_does_not_disturb_the_cache() {
    let doc = double_struck_page();
    let page = doc.page(0).unwrap();
    let before = page.chars().unwrap().len();
    let deduped = page.dedupe_chars(1.0).unwrap();
    assert!(deduped.len() < before, "nothing was removed");
    assert_eq!(
        page.chars().unwrap().len(),
        before,
        "dedupe_chars must not mutate what chars() reports"
    );
}

/// A page that draws `HI` twice, 0.3 pt apart — the fake-bold pattern.
fn double_struck_page() -> pdfraw::Document {
    synthetic::PdfBuilder::new()
        .font(
            "F1",
            synthetic::simple_font("WinAnsiEncoding", 32, &[500; 96]),
        )
        .content(
            "BT /F1 12 Tf 100 700 Td (HI) Tj ET \
             BT /F1 12 Tf 100.3 700 Td (HI) Tj ET",
        )
        .build()
}
