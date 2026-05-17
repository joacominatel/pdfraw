//! Positioned glyph type.
//!
//! A [`Char`] is the unit of output of the PDF parser. Each glyph drawn by
//! the content stream becomes one `Char` with its decoded Unicode text and
//! its position on the page in **top-down** coordinates (origin at the
//! top-left of the page).

use compact_str::CompactString;

/// A single positioned glyph extracted from a PDF page.
///
/// One PDF glyph operator (`Tj`, `TJ`, `'`, `"`) typically produces one or
/// more `Char` instances — one per visible glyph. Ligatures like `ﬁ` are
/// preserved as a single `Char` whose `text` is `"ﬁ"`; they can be expanded
/// to `"fi"` via [`crate::text::ligatures::expand`].
#[derive(Debug, Clone)]
pub struct Char {
    /// The decoded Unicode text for this glyph. Usually a single grapheme;
    /// can be multi-character for ligatures or one-to-many CMap entries.
    pub text: CompactString,
    /// Left edge (page-space, top-down).
    pub x0: f32,
    /// Right edge (page-space).
    pub x1: f32,
    /// Top edge (page-space, top-down — smaller = higher on the page).
    pub top: f32,
    /// Bottom edge (page-space).
    pub bottom: f32,
    /// Cross-document top: `top` plus the cumulative height of previous pages.
    pub doctop: f32,
    /// Effective font size in points.
    pub size: f32,
    /// PDF font name as referenced in the page's font resource dictionary.
    pub fontname: CompactString,
    /// `true` if the glyph is drawn with an upright (non-rotated/sheared)
    /// matrix. Layout extraction only considers upright glyphs.
    pub upright: bool,
}
