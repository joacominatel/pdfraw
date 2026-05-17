//! Char → Word → Line → layout text reconstruction.
//!
//! This module owns the algorithms; backends in [`crate::parser`] feed it
//! a flat slice of [`Char`]s and it produces user-facing text.

use crate::char::Char;
use crate::text::options::TextOptions;
use crate::word::{Word, WordOptions};

/// Cluster a slice of chars into words.
///
/// Phase 2 will implement the full WordExtractor (retroceso, gap, salto de
/// línea). Stub returns an empty vec for now.
pub fn extract_words(_chars: &[Char], _opts: &WordOptions) -> Vec<Word> {
    Vec::new()
}

/// Naive concatenated text: one space between words, one newline between
/// lines. Phase 3.
pub fn extract_text_simple(_chars: &[Char]) -> String {
    String::new()
}

/// Layout-preserving text replicating pdfplumber's algorithm. Phase 4.
pub fn extract_text_layout(_chars: &[Char], _opts: &TextOptions) -> String {
    String::new()
}
