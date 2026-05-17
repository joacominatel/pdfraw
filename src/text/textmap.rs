//! Output-to-source character mapping.
//!
//! [`TextMap`] is the structured output of [`crate::text::extractor::extract_text_layout`]:
//! a sequence of `(output_char, Option<&Char>)` cells where `None` marks an
//! inserted space or newline.

use crate::char::Char;

/// Layout-reconstructed text together with a back-reference to the source
/// chars that produced each output character.
#[derive(Debug)]
pub struct TextMap<'a> {
    /// Each cell is one Unicode scalar value in the output, paired with the
    /// source [`Char`] when one exists. Inserted spacing has `None`.
    pub cells: Vec<(char, Option<&'a Char>)>,
}

impl<'a> TextMap<'a> {
    /// Render the cells as a `String`.
    pub fn as_string(&self) -> String {
        self.cells.iter().map(|(c, _)| *c).collect()
    }
}
