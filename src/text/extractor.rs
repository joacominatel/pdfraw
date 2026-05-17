//! Char → Word → Line → layout text reconstruction.
//!
//! This module owns the algorithms; backends in [`crate::parser`] feed it
//! a flat slice of [`Char`]s and it produces user-facing text.

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

    let mut indices: Vec<usize> = (0..working.len()).collect();
    if !opts.use_text_flow {
        indices.sort_by(|&a, &b| compare_chars(&working[a], &working[b]));
    }

    let ordered: Vec<&Char> = indices.iter().map(|&i| &working[i]).collect();
    let lines = cluster_objects(&ordered, |c| c.top, opts.y_tolerance);

    let mut out = Vec::new();
    for line in lines {
        let mut line: Vec<&Char> = line.into_iter().copied().collect();
        line.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(Ordering::Equal));

        let mut current: Vec<&Char> = Vec::new();
        for c in line {
            if let Some(prev) = current.last() {
                if char_begins_new_word(prev, c, opts) {
                    if let Some(w) = build_word(&current, &working) {
                        out.push(w);
                    }
                    current.clear();
                }
            }
            if !opts.keep_blank_chars && c.text.chars().all(char::is_whitespace) {
                if let Some(w) = build_word(&current, &working) {
                    out.push(w);
                }
                current.clear();
                continue;
            }
            current.push(c);
        }
        if let Some(w) = build_word(&current, &working) {
            out.push(w);
        }
    }

    out
}

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

fn build_word(chars: &[&Char], working: &[Char]) -> Option<Word> {
    let first = *chars.first()?;
    let last = *chars.last()?;
    let text: String = chars.iter().map(|c| c.text.as_str()).collect();
    let x0 = chars.iter().map(|c| c.x0).fold(f32::INFINITY, f32::min);
    let x1 = chars.iter().map(|c| c.x1).fold(f32::NEG_INFINITY, f32::max);
    let top = chars.iter().map(|c| c.top).fold(f32::INFINITY, f32::min);
    let bottom = chars
        .iter()
        .map(|c| c.bottom)
        .fold(f32::NEG_INFINITY, f32::max);

    let char_range = char_range_for(first, last, working);

    Some(Word {
        text: CompactString::from(text.as_str()),
        x0,
        x1,
        top,
        bottom,
        char_range,
    })
}

/// Map a word's first/last char references back to their indices in the
/// `working` slice the parser produced.
///
/// When the working slice equals the input slice (no ligature expansion)
/// these indices match the public `chars()` output 1:1.
fn char_range_for(first: &Char, last: &Char, working: &[Char]) -> std::ops::Range<usize> {
    let first_idx = working
        .iter()
        .position(|c| std::ptr::eq(c, first))
        .unwrap_or(0);
    let last_idx = working
        .iter()
        .rposition(|c| std::ptr::eq(c, last))
        .unwrap_or(first_idx);
    first_idx..(last_idx + 1)
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

    // Bounding box origin: smallest x0 and smallest top across all chars
    // (matches pdfplumber's default layout_bbox = page bbox of chars).
    let x_origin = chars.iter().map(|c| c.x0).fold(f32::INFINITY, f32::min);
    let y_origin = chars.iter().map(|c| c.top).fold(f32::INFINITY, f32::min);

    // Group words back into lines using y_tolerance, preserving the
    // top-to-bottom order produced by extract_words.
    let mut lines: Vec<Vec<&Word>> = Vec::new();
    for w in &words {
        match lines.last_mut() {
            Some(line) if same_line(line[0], w, opts.y_tolerance) => line.push(w),
            _ => lines.push(vec![w]),
        }
    }
    // Ensure top-ascending order across lines (extract_words already sorts
    // by top, but defensive sort costs nothing).
    lines.sort_by(|a, b| a[0].top.partial_cmp(&b[0].top).unwrap_or(Ordering::Equal));

    let mut out = String::new();
    let mut newlines_so_far: i32 = 0;

    for (i, line) in lines.iter().enumerate() {
        let y_dist = (line[0].top - y_origin) / opts.y_density;
        let target_row = y_dist.round() as i32;
        let minimum = if i > 0 { 1 } else { 0 };
        let needed_newlines = (target_row - newlines_so_far).max(minimum);
        out.extend(std::iter::repeat_n('\n', needed_newlines as usize));
        newlines_so_far += needed_newlines;

        let mut col: i32 = 0;
        for (j, w) in line.iter().enumerate() {
            let x_dist = (w.x0 - x_origin) / opts.x_density;
            let target_col = x_dist.round() as i32;
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
        let words = extract_words(&[], &WordOptions::default());
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
