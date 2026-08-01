//! Word clustering: groups of adjacent [`Char`](crate::Char)s on the same line.

use compact_str::CompactString;

/// A word clustered from one or more adjacent glyphs on the same line.
#[derive(Debug, Clone)]
pub struct Word {
    /// Concatenated text.
    pub text: CompactString,
    /// Left edge of the leftmost char.
    pub x0: f32,
    /// Right edge of the rightmost char.
    pub x1: f32,
    /// Top edge.
    pub top: f32,
    /// Bottom edge.
    pub bottom: f32,
    /// Half-open index range into the page's char slice spanning this word's
    /// source glyphs, useful for mapping output back to them.
    ///
    /// The glyphs need not be contiguous: a content stream may emit them out
    /// of reading order, in which case the range also covers whatever sits
    /// between the word's lowest and highest index.
    pub char_range: std::ops::Range<usize>,
}

/// Options that control how [`crate::text::extractor::extract_words`]
/// clusters chars into words.
#[derive(Debug, Clone)]
pub struct WordOptions {
    /// Maximum horizontal gap (in points) between adjacent glyphs of the
    /// same word.
    pub x_tolerance: f32,
    /// Maximum vertical difference (in points) for two glyphs to be
    /// considered on the same line.
    pub y_tolerance: f32,
    /// When `Some(ratio)`, x-tolerance is dynamic: `ratio * char.size`.
    /// Overrides the fixed `x_tolerance` if set.
    pub x_tolerance_ratio: Option<f32>,
    /// If `true`, preserve literal space characters present in the PDF
    /// content stream rather than treating them as word separators.
    pub keep_blank_chars: bool,
    /// If `true`, walk chars in their PDF stream order instead of sorting
    /// by `(top, x0)`.
    pub use_text_flow: bool,
    /// If `true`, expand multi-codepoint ligatures (`ﬁ → fi`) before
    /// clustering.
    pub expand_ligatures: bool,
}

impl Default for WordOptions {
    /// Defaults match pdfplumber's `extract_words()`.
    fn default() -> Self {
        Self {
            x_tolerance: 3.0,
            y_tolerance: 3.0,
            x_tolerance_ratio: None,
            keep_blank_chars: false,
            use_text_flow: false,
            expand_ligatures: false,
        }
    }
}
