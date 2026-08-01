//! Char → Word → Line → layout text reconstruction.
//!
//! This module owns the algorithms; the internal parser backend feeds it a
//! flat slice of [`Char`]s and it produces user-facing text.

use crate::char::Char;
use crate::text::cluster::cluster_objects;
use crate::text::ligatures::expand;
use crate::text::options::TextOptions;
use crate::word::{Word, WordOptions};
use compact_str::CompactString;
use std::borrow::Cow;
use std::cmp::Ordering;

/// Cluster a slice of chars into words.
///
/// Replicates pdfplumber's `WordExtractor`:
/// 1. Sort chars by `(top, x0)` (unless `opts.use_text_flow == true`).
/// 2. Cluster by `top` with `y_tolerance` → lines.
/// 3. Within each line, walk left-to-right and start a new word when the
///    horizontal gap exceeds `x_tolerance`, when there's a backward x move,
///    or when the line `top` differs.
///
/// When `opts.expand_ligatures == false` the input slice is borrowed and no
/// allocation happens beyond the output `Vec<Word>`.
///
/// # Examples
///
/// ```
/// # use pdfraw::{Char, WordOptions};
/// # use pdfraw::text::extractor::extract_words;
/// # use compact_str::CompactString;
/// let chars = vec![
///     Char { text: CompactString::from("H"), x0: 0.0, x1: 5.0, top: 0.0,
///            bottom: 10.0, doctop: 0.0, size: 10.0,
///            fontname: CompactString::from("F1"), upright: true },
///     Char { text: CompactString::from("i"), x0: 5.0, x1: 8.0, top: 0.0,
///            bottom: 10.0, doctop: 0.0, size: 10.0,
///            fontname: CompactString::from("F1"), upright: true },
/// ];
/// let words = extract_words(&chars, &WordOptions::default());
/// assert_eq!(words[0].text.as_str(), "Hi");
/// ```
pub fn extract_words(chars: &[Char], opts: &WordOptions) -> Vec<Word> {
    if chars.is_empty() {
        return Vec::new();
    }

    // Borrow the input unless ligature expansion forces us to allocate.
    let working: Cow<'_, [Char]> = if opts.expand_ligatures {
        Cow::Owned(chars.iter().map(expand_char).collect())
    } else {
        Cow::Borrowed(chars)
    };

    // `Char::upright` is documented as a layout filter — "layout extraction
    // only considers upright glyphs" — so a sideways glyph must not be
    // clustered into the horizontal line it happens to overlap.
    let mut indices: Vec<usize> = (0..working.len()).filter(|&i| working[i].upright).collect();
    if !opts.use_text_flow {
        indices.sort_by(|&a, &b| compare_chars(&working[a], &working[b]));
    }
    if indices.is_empty() {
        return Vec::new();
    }

    // Carry each char's index in `working` alongside the reference. The
    // index is what `Word::char_range` is built from; recovering it later by
    // scanning for a matching pointer would be both quadratic and wrong when
    // the positional sort reorders a word's chars.
    let ordered: Vec<Indexed<'_>> = indices.iter().map(|&i| (i, &working[i])).collect();
    let lines = cluster_objects(&ordered, |(_, c)| c.top, opts.y_tolerance);

    let mut out = Vec::new();
    for line in lines {
        let mut line: Vec<Indexed<'_>> = line.into_iter().copied().collect();
        // `use_text_flow` means "walk the glyphs as the PDF drew them". A
        // per-line sort by x0 undid that just as thoroughly as the global
        // sort, so the option only half worked.
        if !opts.use_text_flow {
            line.sort_by(|(_, a), (_, b)| a.x0.partial_cmp(&b.x0).unwrap_or(Ordering::Equal));
        } else {
            line.sort_by_key(|(i, _)| *i);
        }

        let mut current: Vec<Indexed<'_>> = Vec::new();
        for (idx, c) in line {
            if let Some((_, prev)) = current.last()
                && char_begins_new_word(prev, c, opts)
            {
                if let Some(w) = build_word(&current) {
                    out.push(w);
                }
                current.clear();
            }
            // `chars().all(..)` is vacuously true for an empty string, so a
            // zero-length Char used to read as blank and split the word it
            // sat inside.
            if !opts.keep_blank_chars
                && !c.text.is_empty()
                && c.text.chars().all(char::is_whitespace)
            {
                if let Some(w) = build_word(&current) {
                    out.push(w);
                }
                current.clear();
                continue;
            }
            current.push((idx, c));
        }
        if let Some(w) = build_word(&current) {
            out.push(w);
        }
    }

    out
}

/// A char paired with its index in the working slice.
type Indexed<'a> = (usize, &'a Char);

fn char_begins_new_word(prev: &Char, curr: &Char, opts: &WordOptions) -> bool {
    if (prev.top - curr.top).abs() > opts.y_tolerance {
        return true;
    }
    if curr.x0 + opts.x_tolerance < prev.x0 {
        // Strong backward jump — new word.
        return true;
    }
    let x_tol = opts
        .x_tolerance_ratio
        .map(|r| r * prev.size)
        .unwrap_or(opts.x_tolerance);
    curr.x0 > prev.x1 + x_tol
}

fn build_word(chars: &[Indexed<'_>]) -> Option<Word> {
    if chars.is_empty() {
        return None;
    }
    let text: String = chars.iter().map(|(_, c)| c.text.as_str()).collect();
    // A word made only of non-finite coordinates keeps NaN rather than being
    // laundered into ±infinity, which would then saturate the layout grid.
    let x0 = min_finite(chars.iter().map(|(_, c)| c.x0)).unwrap_or(f32::NAN);
    let x1 = max_finite(chars.iter().map(|(_, c)| c.x1)).unwrap_or(f32::NAN);
    let top = min_finite(chars.iter().map(|(_, c)| c.top)).unwrap_or(f32::NAN);
    let bottom = max_finite(chars.iter().map(|(_, c)| c.bottom)).unwrap_or(f32::NAN);

    // The word's chars need not be contiguous in stream order, so the range
    // spans from the lowest to the highest index they occupy.
    let lo = chars.iter().map(|(i, _)| *i).min()?;
    let hi = chars.iter().map(|(i, _)| *i).max()?;

    Some(Word {
        text: CompactString::from(text.as_str()),
        x0,
        x1,
        top,
        bottom,
        char_range: lo..(hi + 1),
    })
}

fn expand_char(c: &Char) -> Char {
    let mut clone = c.clone();
    let expanded = expand(c.text.as_str());
    clone.text = CompactString::from(expanded.as_str());
    clone
}

fn compare_chars(a: &Char, b: &Char) -> Ordering {
    a.top
        .partial_cmp(&b.top)
        .unwrap_or(Ordering::Equal)
        .then(a.x0.partial_cmp(&b.x0).unwrap_or(Ordering::Equal))
}

/// Naive concatenated text: one space between words, one newline between
/// lines. Whitespace within a line is preserved as single spaces.
pub fn extract_text_simple(chars: &[Char]) -> String {
    let opts = WordOptions::default();
    let words = extract_words(chars, &opts);
    if words.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let mut prev_top: Option<f32> = None;
    for (i, w) in words.iter().enumerate() {
        match prev_top {
            Some(t) if (t - w.top).abs() > opts.y_tolerance => out.push('\n'),
            Some(_) if i > 0 => out.push(' '),
            _ => {}
        }
        out.push_str(w.text.as_str());
        prev_top = Some(w.top);
    }
    out
}

/// Layout-preserving text replicating pdfplumber's algorithm.
///
/// Reconstructs a monospaced virtual grid where the spatial relationship
/// between glyphs is preserved through inserted spaces and newlines:
/// - each output column is `x_density` points wide (default 7.25);
/// - each output line is `y_density` points tall (default 13.0);
/// - for every word, we insert enough leading spaces to align its `x0` to
///   the nearest virtual column;
/// - for every line, we insert enough leading newlines to align its `top`
///   to the nearest virtual row.
pub fn extract_text_layout(chars: &[Char], opts: &TextOptions) -> String {
    if chars.is_empty() {
        return String::new();
    }

    let word_opts = WordOptions {
        x_tolerance: opts.x_tolerance,
        y_tolerance: opts.y_tolerance,
        x_tolerance_ratio: opts.x_tolerance_ratio,
        keep_blank_chars: opts.keep_blank_chars,
        use_text_flow: opts.use_text_flow,
        expand_ligatures: opts.expand_ligatures,
    };
    let words = extract_words(chars, &word_opts);
    if words.is_empty() {
        return String::new();
    }

    // A density of zero (or a negative or non-finite one) would divide by
    // zero, saturate the grid position to i32::MAX and try to allocate
    // gigabytes of padding. Fall back to the pdfplumber defaults instead.
    let x_density = sane_density(opts.x_density, DEFAULT_X_DENSITY, "x_density");
    let y_density = sane_density(opts.y_density, DEFAULT_Y_DENSITY, "y_density");

    // Bounding box origin: smallest x0 and smallest top across all chars
    // (matches pdfplumber's default layout_bbox = page bbox of chars).
    let x_origin = min_finite(chars.iter().map(|c| c.x0)).unwrap_or(0.0);
    let y_origin = min_finite(chars.iter().map(|c| c.top)).unwrap_or(0.0);

    // Group words back into lines using y_tolerance, preserving the
    // top-to-bottom order produced by extract_words.
    //
    // The comparison is against the *previous* word, not the first one in the
    // line. `extract_words` clusters chars by chaining — each within
    // y_tolerance of the one before — so measuring from `line[0]` here made
    // the two stages disagree: a baseline drifting a couple of points per
    // word was one line to the word extractor and several to the layout.
    let mut lines: Vec<Vec<&Word>> = Vec::new();
    for w in &words {
        match lines.last_mut() {
            Some(line) if same_line(line[line.len() - 1], w, opts.y_tolerance) => line.push(w),
            _ => lines.push(vec![w]),
        }
    }
    // Ensure top-ascending order across lines (extract_words already sorts
    // by top, but defensive sort costs nothing).
    lines.sort_by(|a, b| a[0].top.partial_cmp(&b[0].top).unwrap_or(Ordering::Equal));

    let mut out = String::new();
    let mut newlines_so_far: i32 = 0;

    for (i, line) in lines.iter().enumerate() {
        let y_dist = (line[0].top - y_origin) / y_density;
        let target_row = grid_position(y_dist, MAX_GRID_ROWS);
        let minimum = if i > 0 { 1 } else { 0 };
        let needed_newlines = (target_row - newlines_so_far).max(minimum);
        out.extend(std::iter::repeat_n('\n', needed_newlines as usize));
        newlines_so_far += needed_newlines;

        let mut col: i32 = 0;
        for (j, w) in line.iter().enumerate() {
            let x_dist = (w.x0 - x_origin) / x_density;
            let target_col = grid_position(x_dist, MAX_GRID_COLUMNS);
            let minimum = if j > 0 { 1 } else { 0 };
            let needed_spaces = (target_col - col).max(minimum);
            out.extend(std::iter::repeat_n(' ', needed_spaces as usize));
            col += needed_spaces;

            out.push_str(w.text.as_str());
            col += w.text.chars().count() as i32;
        }
    }
    out
}

/// Widest virtual grid we will pad out to, in columns.
///
/// A US Letter page is about 84 columns at the default density, so this is
/// two orders of magnitude of headroom for legitimate documents. Its real
/// job is to stop a single glyph claiming `x0 = 1e8` from turning a two-char
/// page into tens of megabytes of spaces.
const MAX_GRID_COLUMNS: i32 = 20_000;

/// Tallest virtual grid we will pad out to, in rows. Same reasoning as
/// [`MAX_GRID_COLUMNS`]: a Letter page is about 61 rows by default.
const MAX_GRID_ROWS: i32 = 20_000;

const DEFAULT_X_DENSITY: f32 = 7.25;
const DEFAULT_Y_DENSITY: f32 = 13.0;

/// Convert a distance in grid units into a clamped, non-negative index.
///
/// `as i32` already saturates on infinity and yields 0 on NaN, but the raw
/// value can still be astronomically large, so it is clamped to `max`.
fn grid_position(distance: f32, max: i32) -> i32 {
    if distance.is_nan() {
        return 0;
    }
    (distance.round() as i32).clamp(0, max)
}

/// Replace a density that cannot produce a usable grid with `fallback`.
fn sane_density(value: f32, fallback: f32, name: &str) -> f32 {
    if value.is_finite() && value > 0.0 {
        return value;
    }
    log::warn!("{name} = {value} is not a usable grid density; falling back to {fallback}");
    fallback
}

/// Smallest finite value in `values`, or `None` if there is no finite one.
///
/// `f32::min` returns the non-NaN operand, so folding from `f32::INFINITY`
/// silently turns an all-NaN input into `+inf` — which then saturates the
/// layout grid. This keeps non-finite values out instead of laundering them.
fn min_finite(values: impl Iterator<Item = f32>) -> Option<f32> {
    values
        .filter(|v| v.is_finite())
        .fold(None, |acc: Option<f32>, v| {
            Some(acc.map_or(v, |a| a.min(v)))
        })
}

/// Largest finite value in `values`, or `None` if there is no finite one.
fn max_finite(values: impl Iterator<Item = f32>) -> Option<f32> {
    values
        .filter(|v| v.is_finite())
        .fold(None, |acc: Option<f32>, v| {
            Some(acc.map_or(v, |a| a.max(v)))
        })
}

fn same_line(a: &Word, b: &Word, y_tol: f32) -> bool {
    (a.top - b.top).abs() <= y_tol
}

#[cfg(test)]
mod tests {
    use super::*;
    use compact_str::CompactString;

    fn mk(text: &str, x0: f32, x1: f32, top: f32, bottom: f32) -> Char {
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

    #[test]
    fn empty_chars_returns_no_words() {
        let chars: Vec<Char> = Vec::new();
        let words = extract_words(&chars, &WordOptions::default());
        assert!(words.is_empty());
    }

    #[test]
    fn single_word_groups_adjacent_chars() {
        let chars = vec![mk("H", 0.0, 5.0, 10.0, 22.0), mk("i", 5.0, 8.0, 10.0, 22.0)];
        let words = extract_words(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text.as_str(), "Hi");
    }

    #[test]
    fn horizontal_gap_splits_words() {
        let chars = vec![
            mk("A", 0.0, 5.0, 10.0, 22.0),
            mk("B", 20.0, 25.0, 10.0, 22.0), // gap > 3pt default
        ];
        let words = extract_words(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
    }

    #[test]
    fn different_lines_produce_separate_words() {
        let chars = vec![
            mk("A", 0.0, 5.0, 10.0, 22.0),
            mk("B", 5.0, 10.0, 50.0, 62.0), // far below
        ];
        let words = extract_words(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
    }

    #[test]
    fn extract_text_simple_inserts_newline_between_lines() {
        let chars = vec![mk("A", 0.0, 5.0, 10.0, 22.0), mk("B", 0.0, 5.0, 50.0, 62.0)];
        let text = extract_text_simple(&chars);
        assert_eq!(text, "A\nB");
    }

    #[test]
    fn layout_aligns_right_column_with_spaces() {
        // Two words on the same line: "Total" at x=0, "$100" far right at x=290
        // With default x_density=7.25, 290/7.25 ≈ 40 columns → ~35 spaces.
        let mut chars = vec![];
        for (i, ch) in "Total".chars().enumerate() {
            let x = i as f32 * 6.0;
            chars.push(mk(&ch.to_string(), x, x + 6.0, 10.0, 22.0));
        }
        for (i, ch) in "$100".chars().enumerate() {
            let x = 290.0 + i as f32 * 6.0;
            chars.push(mk(&ch.to_string(), x, x + 6.0, 10.0, 22.0));
        }
        let opts = TextOptions::pdfplumber_defaults();
        let text = extract_text_layout(&chars, &opts);
        assert!(text.starts_with("Total"));
        assert!(text.contains("$100"));
        // Number of spaces between "Total" and "$100" should be roughly
        // (290 - 30) / 7.25 ≈ 35 → at least 30 spaces.
        let gap_len = text["Total".len()..text.find("$100").unwrap()].len();
        assert!(gap_len > 25, "gap was {gap_len} chars: {text:?}");
    }

    #[test]
    fn expand_ligatures_option_replaces_fi_with_two_chars() {
        let chars = vec![mk("\u{FB01}", 0.0, 5.0, 10.0, 22.0)];
        let opts = WordOptions {
            expand_ligatures: true,
            ..WordOptions::default()
        };
        let words = extract_words(&chars, &opts);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text.as_str(), "fi");
    }

    #[test]
    fn layout_preserves_two_lines_with_vertical_gap() {
        // Header at top=10, body at top=50 (40pt apart, ≈3 virtual rows).
        let chars = vec![mk("H", 0.0, 5.0, 10.0, 22.0), mk("B", 0.0, 5.0, 50.0, 62.0)];
        let opts = TextOptions::pdfplumber_defaults();
        let text = extract_text_layout(&chars, &opts);
        // (50 - 10) / 13 = 3.07 → 3 newlines.
        assert!(text.matches('\n').count() >= 2, "got {text:?}");
        assert!(text.contains('H') && text.contains('B'));
    }
}
