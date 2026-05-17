//! Font-related parsing helpers.
//!
//! - [`glyph_names`] — small Adobe glyph-name → Unicode lookup.
//! - [`differences`] — apply a font's `/Differences` encoding override.
//! - [`widths`] — parse Type0 `/W` arrays.

pub(crate) mod differences;
pub(crate) mod glyph_names;
pub(crate) mod widths;
