//! Adversarial tests for the clustering / layout reconstruction stage.
//!
//! Every test here is an attempt to break `extract_words`,
//! `extract_text_simple`, `extract_text_layout`, `cluster_objects` or the
//! geometry primitives with degenerate, hostile or simply unusual input.
//! Tests marked `#[ignore = "BUG: ..."]` reproduce a real defect.

mod synthetic;

use pdfraw::geom::Matrix;
use pdfraw::prelude::*;
use pdfraw::text::cluster::cluster_objects;
use pdfraw::text::extractor::{extract_text_layout, extract_text_simple, extract_words};
use synthetic::{ch, with_timeout};

// ---------------------------------------------------------------------------
// Word.char_range
// ---------------------------------------------------------------------------

#[test]
fn char_range_start_is_not_after_end_when_chars_are_in_reverse_stream_order() {
    // Same line, contiguous boxes, but emitted right-to-left by the PDF.
    let chars = vec![
        ch("C", 20.0, 30.0, 10.0, 22.0),
        ch("B", 10.0, 20.0, 10.0, 22.0),
        ch("A", 0.0, 10.0, 10.0, 22.0),
    ];
    let words = extract_words(&chars, &WordOptions::default());
    assert_eq!(words.len(), 1, "expected one word, got {words:?}");
    let r = &words[0].char_range;
    assert!(
        r.start <= r.end,
        "char_range must be a valid range, got {r:?}"
    );
}

#[test]
fn char_range_indexes_source_slice_when_chars_are_in_reverse_stream_order() {
    let chars = vec![
        ch("C", 20.0, 30.0, 10.0, 22.0),
        ch("B", 10.0, 20.0, 10.0, 22.0),
        ch("A", 0.0, 10.0, 10.0, 22.0),
    ];
    let words = extract_words(&chars, &WordOptions::default());
    let range = words[0].char_range.clone();
    let slice = chars.get(range.clone());
    assert!(
        slice.is_some(),
        "char_range {range:?} does not index the source chars"
    );
    assert_eq!(
        slice.unwrap().len(),
        3,
        "char_range should cover the word's three chars"
    );
}

// ---------------------------------------------------------------------------
// NaN / non-finite coordinates
// ---------------------------------------------------------------------------

#[test]
fn cluster_objects_returns_all_items_when_keys_contain_nan() {
    // A comparator that answers `Equal` for NaN is not a total order; the
    // standard sort is allowed to panic on such comparators.
    let mut keys: Vec<f32> = Vec::new();
    for i in 0..64 {
        keys.push(if i % 3 == 0 { f32::NAN } else { i as f32 });
    }
    let groups = cluster_objects(&keys, |k| *k, 3.0);
    let total: usize = groups.iter().map(|g| g.len()).sum();
    assert_eq!(total, keys.len(), "clustering must not lose items");
}

#[test]
fn extract_words_keeps_every_glyph_when_tops_contain_nan() {
    let mut chars = Vec::new();
    for i in 0..64 {
        let top = if i % 3 == 0 { f32::NAN } else { i as f32 };
        chars.push(ch(
            "x",
            i as f32 * 12.0,
            i as f32 * 12.0 + 10.0,
            top,
            top + 12.0,
        ));
    }
    let words = extract_words(&chars, &WordOptions::default());
    let emitted: usize = words.iter().map(|w| w.text.chars().count()).sum();
    assert_eq!(emitted, chars.len(), "every glyph should end up in a word");
}

#[test]
fn extract_words_does_not_launder_nan_coordinates_into_infinity() {
    let chars = vec![ch("A", f32::NAN, f32::NAN, f32::NAN, f32::NAN)];
    let words = extract_words(&chars, &WordOptions::default());
    assert_eq!(words.len(), 1);
    let w = &words[0];
    assert!(
        !w.x0.is_infinite() && !w.x1.is_infinite(),
        "NaN x became infinite: x0={} x1={}",
        w.x0,
        w.x1
    );
    assert!(
        !w.top.is_infinite() && !w.bottom.is_infinite(),
        "NaN y became infinite: top={} bottom={}",
        w.top,
        w.bottom
    );
}

// ---------------------------------------------------------------------------
// Unbounded output: the layout grid is never clipped to the page
// ---------------------------------------------------------------------------

#[test]
fn extract_text_layout_bounds_output_when_a_glyph_lies_far_off_the_page() {
    // Two glyphs. The second one claims x0 = 1e8 points (~35 km).
    let chars = vec![
        ch("A", 0.0, 10.0, 10.0, 22.0),
        ch("B", 1e8, 1e8 + 10.0, 10.0, 22.0),
    ];
    let out = with_timeout(20, move || {
        extract_text_layout(&chars, &TextOptions::pdfplumber_defaults())
    })
    .expect("layout extraction must terminate");
    assert!(
        out.len() < 100_000,
        "two glyphs produced {} output chars",
        out.len()
    );
}

#[test]
fn extract_text_layout_terminates_when_y_density_is_zero() {
    let chars = vec![
        ch("A", 0.0, 10.0, 10.0, 22.0),
        ch("B", 0.0, 10.0, 50.0, 62.0),
    ];
    let opts = TextOptions::builder().y_density(0.0).build();
    let out = with_timeout(5, move || extract_text_layout(&chars, &opts))
        .expect("zero y_density must not wedge the extractor");
    assert!(
        out.len() < 100_000,
        "zero y_density produced {} output chars",
        out.len()
    );
}

#[test]
fn extract_text_layout_terminates_when_x_density_is_zero() {
    let chars = vec![
        ch("A", 0.0, 10.0, 10.0, 22.0),
        ch("B", 40.0, 50.0, 10.0, 22.0),
    ];
    let opts = TextOptions::builder().x_density(0.0).build();
    let out = with_timeout(5, move || extract_text_layout(&chars, &opts))
        .expect("zero x_density must not wedge the extractor");
    assert!(
        out.len() < 100_000,
        "zero x_density produced {} output chars",
        out.len()
    );
}

// ---------------------------------------------------------------------------
// Option contracts that are not honoured
// ---------------------------------------------------------------------------

#[test]
#[ignore = "BUG: use_text_flow only skips the global sort; chars are still sorted by x0 inside every line, so stream order is lost"]
fn extract_words_preserves_stream_order_when_use_text_flow_is_set() {
    // The PDF drew "B" before "A" on the same line. With use_text_flow the
    // extractor must not reorder them.
    let chars = vec![
        ch("B", 10.0, 20.0, 10.0, 22.0),
        ch("A", 0.0, 10.0, 10.0, 22.0),
    ];
    let opts = WordOptions {
        use_text_flow: true,
        ..WordOptions::default()
    };
    let words = extract_words(&chars, &opts);
    let joined: String = words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(joined, "BA", "stream order was not preserved: {words:?}");
}

#[test]
#[ignore = "BUG: `Char::upright` is documented as a layout filter but extract_words/extract_text_layout never read it, so rotated glyphs pollute horizontal lines"]
fn extract_words_skips_non_upright_glyphs() {
    let mut sideways = ch("R", 30.0, 40.0, 10.0, 22.0);
    sideways.upright = false;
    let chars = vec![ch("A", 0.0, 10.0, 10.0, 22.0), sideways];
    let words = extract_words(&chars, &WordOptions::default());
    let joined: String = words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(joined, "A", "non-upright glyph leaked into the words");
}

#[test]
fn extract_words_ignores_zero_length_char_between_glyphs() {
    let chars = vec![
        ch("A", 0.0, 10.0, 10.0, 22.0),
        ch("", 10.0, 10.0, 10.0, 22.0),
        ch("B", 10.0, 20.0, 10.0, 22.0),
    ];
    let words = extract_words(&chars, &WordOptions::default());
    let joined: String = words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(joined, "AB");
    assert_eq!(words.len(), 1, "empty glyph split the word: {words:?}");
}

// ---------------------------------------------------------------------------
// Line-splitting consistency between the two stages
// ---------------------------------------------------------------------------

#[test]
#[ignore = "BUG: extract_words clusters lines by chaining (each char within y_tolerance of the previous) but extract_text_layout regroups against the first word only, so a drifting baseline is split into extra output lines"]
fn extract_text_layout_keeps_one_line_when_word_extraction_saw_one_line() {
    // Baseline drifts by 2.5pt per word — inside the 3pt tolerance chain.
    let chars = vec![
        ch("A", 0.0, 10.0, 0.0, 12.0),
        ch("B", 50.0, 60.0, 2.5, 14.5),
        ch("C", 100.0, 110.0, 5.0, 17.0),
    ];
    let opts = TextOptions::pdfplumber_defaults();
    let words = extract_words(
        &chars,
        &WordOptions {
            y_tolerance: opts.y_tolerance,
            ..WordOptions::default()
        },
    );
    assert_eq!(words.len(), 3, "expected three words");

    let layout = extract_text_layout(&chars, &opts);
    assert_eq!(
        layout.lines().count(),
        1,
        "chars clustered as one line were laid out on several lines: {layout:?}"
    );
}

// ---------------------------------------------------------------------------
// Degenerate tolerances
// ---------------------------------------------------------------------------

#[test]
fn extract_words_keeps_every_glyph_when_y_tolerance_is_negative() {
    let chars = vec![
        ch("A", 0.0, 10.0, 10.0, 22.0),
        ch("B", 10.0, 20.0, 10.0, 22.0),
    ];
    let opts = WordOptions {
        y_tolerance: -5.0,
        ..WordOptions::default()
    };
    let words = extract_words(&chars, &opts);
    let emitted: usize = words.iter().map(|w| w.text.chars().count()).sum();
    assert_eq!(emitted, 2, "glyphs were dropped: {words:?}");
}

#[test]
fn extract_words_keeps_every_glyph_when_tolerances_are_nan() {
    let chars = vec![
        ch("A", 0.0, 10.0, 10.0, 22.0),
        ch("B", 10.0, 20.0, 10.0, 22.0),
    ];
    let opts = WordOptions {
        x_tolerance: f32::NAN,
        y_tolerance: f32::NAN,
        ..WordOptions::default()
    };
    let words = extract_words(&chars, &opts);
    let emitted: usize = words.iter().map(|w| w.text.chars().count()).sum();
    assert_eq!(emitted, 2, "glyphs were dropped: {words:?}");
}

#[test]
fn keep_blank_chars_preserves_a_literal_space_inside_a_word() {
    let chars = vec![
        ch("A", 0.0, 10.0, 10.0, 22.0),
        ch(" ", 10.0, 15.0, 10.0, 22.0),
        ch("B", 15.0, 25.0, 10.0, 22.0),
    ];
    let opts = WordOptions {
        keep_blank_chars: true,
        ..WordOptions::default()
    };
    let words = extract_words(&chars, &opts);
    let joined: String = words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(joined, "A B", "blank char was not kept: {words:?}");
}

#[test]
fn extract_text_simple_returns_empty_string_when_all_chars_are_blank() {
    let chars = vec![
        ch(" ", 0.0, 5.0, 10.0, 22.0),
        ch(" ", 5.0, 10.0, 10.0, 22.0),
    ];
    assert_eq!(extract_text_simple(&chars), "");
}

// ---------------------------------------------------------------------------
// Geometry primitives
// ---------------------------------------------------------------------------

#[test]
fn is_upright_returns_false_when_a_tiny_matrix_is_rotated() {
    let s = std::f32::consts::FRAC_1_SQRT_2 * 1e-8;
    let m = Matrix::new(s, s, -s, s, 0.0, 0.0);
    assert!(!m.is_upright(), "45°-rotated matrix reported as upright");
}

#[test]
fn is_upright_returns_false_when_vertical_scale_is_zero() {
    let m = Matrix::new(1.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    assert!(!m.is_upright(), "degenerate matrix reported as upright");
}

#[test]
fn is_upright_returns_false_when_matrix_contains_nan() {
    let m = Matrix::new(f32::NAN, 0.0, 0.0, 1.0, 0.0, 0.0);
    assert!(!m.is_upright(), "NaN matrix reported as upright");
}

#[test]
fn bbox_merge_keeps_finite_bounds_when_one_box_is_nan() {
    let nan = pdfraw::geom::BBox::new(f32::NAN, f32::NAN, f32::NAN, f32::NAN);
    let ok = pdfraw::geom::BBox::new(0.0, 0.0, 10.0, 10.0);
    let merged = nan.merge(&ok);
    assert!(
        merged.width().is_finite() && merged.height().is_finite(),
        "merge produced {merged:?}"
    );
}
