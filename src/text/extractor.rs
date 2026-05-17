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
use std::cmp::Ordering;

/// Cluster a slice of chars into words.
///
/// Replicates pdfplumber's [`WordExtractor`]:
/// 1. Sort chars by `(top, x0)` (unless `use_text_flow=true`).
/// 2. Cluster by `top` with `y_tolerance` → lines.
/// 3. Within each line, walk left-to-right and start a new word when the
///    horizontal gap exceeds `x_tolerance`, when there's a backward x move,
///    or when the line top differs.
pub fn extract_words(chars: &[Char], opts: &WordOptions) -> Vec<Word> {
    if chars.is_empty() {
        return Vec::new();
    }

    // Optionally expand ligatures before clustering. We work on a borrowed
    // view so the public `Char` slice is untouched.
    let working: Vec<Char> = if opts.expand_ligatures {
        chars.iter().map(expand_char).collect()
    } else {
        chars.to_vec()
    };

    // Sort indices for stable positioning.
    let mut indices: Vec<usize> = (0..working.len()).collect();
    if !opts.use_text_flow {
        indices.sort_by(|&a, &b| compare_chars(&working[a], &working[b]));
    }

    let ordered: Vec<&Char> = indices.iter().map(|&i| &working[i]).collect();

    // Cluster into lines by top.
    let lines = cluster_objects(&ordered, |c| c.top, opts.y_tolerance);

    let mut out = Vec::new();
    for line in lines {
        let mut line: Vec<&Char> = line.into_iter().copied().collect();
        line.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(Ordering::Equal));

        let mut current: Vec<&Char> = Vec::new();
        for c in line {
            if let Some(prev) = current.last() {
                if char_begins_new_word(prev, c, opts) {
                    if let Some(w) = build_word(&current, chars, &working, opts.expand_ligatures) {
                        out.push(w);
                    }
                    current.clear();
                }
            }
            // Skip blank chars unless keep_blank_chars is set.
            if !opts.keep_blank_chars && c.text.chars().all(char::is_whitespace) {
                // Whitespace acts as a word boundary too.
                if let Some(w) = build_word(&current, chars, &working, opts.expand_ligatures) {
                    out.push(w);
                }
                current.clear();
                continue;
            }
            current.push(c);
        }
        if let Some(w) = build_word(&current, chars, &working, opts.expand_ligatures) {
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
    let x_tol = opts.x_tolerance_ratio.map(|r| r * prev.size).unwrap_or(opts.x_tolerance);
    curr.x0 > prev.x1 + x_tol
}

fn build_word(
    chars: &[&Char],
    original: &[Char],
    working: &[Char],
    expanded: bool,
) -> Option<Word> {
    if chars.is_empty() {
        return None;
    }
    let mut text = String::new();
    for c in chars {
        text.push_str(c.text.as_str());
    }
    let x0 = chars.iter().map(|c| c.x0).fold(f32::INFINITY, f32::min);
    let x1 = chars.iter().map(|c| c.x1).fold(f32::NEG_INFINITY, f32::max);
    let top = chars.iter().map(|c| c.top).fold(f32::INFINITY, f32::min);
    let bottom = chars.iter().map(|c| c.bottom).fold(f32::NEG_INFINITY, f32::max);

    let char_range = char_range_for(chars, original, working, expanded);

    Some(Word {
        text: CompactString::from(text.as_str()),
        x0,
        x1,
        top,
        bottom,
        char_range,
    })
}

fn char_range_for(
    word_chars: &[&Char],
    original: &[Char],
    working: &[Char],
    expanded: bool,
) -> std::ops::Range<usize> {
    // When we work on the un-expanded slice, indices into `working` line up
    // 1:1 with `original`. With ligature expansion, indices are over the
    // expanded slice — we approximate by mapping the first/last char in the
    // working slice that produced the same coordinates as the word
    // endpoints. For the MVP this is good enough to round-trip simple PDFs.
    let _ = (original, expanded);
    let first = working
        .iter()
        .position(|c| std::ptr::eq(c, word_chars[0] as *const Char))
        .unwrap_or(0);
    let last = working
        .iter()
        .rposition(|c| std::ptr::eq(c, word_chars[word_chars.len() - 1] as *const Char))
        .unwrap_or(first);
    first..(last + 1)
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
/// Stub for phase 4. Falls back to [`extract_text_simple`].
pub fn extract_text_layout(chars: &[Char], _opts: &TextOptions) -> String {
    extract_text_simple(chars)
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
        let chars = vec![
            mk("H", 0.0, 5.0, 10.0, 22.0),
            mk("i", 5.0, 8.0, 10.0, 22.0),
        ];
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
        let chars = vec![
            mk("A", 0.0, 5.0, 10.0, 22.0),
            mk("B", 0.0, 5.0, 50.0, 62.0),
        ];
        let text = extract_text_simple(&chars);
        assert_eq!(text, "A\nB");
    }
}
