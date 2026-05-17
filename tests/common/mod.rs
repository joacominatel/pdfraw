//! Shared test fixtures.
//!
//! Builds small synthetic PDFs with `printpdf` so tests don't depend on
//! committed binaries.

use printpdf::{BuiltinFont, Mm, PdfDocument};

/// Build a minimal one-page PDF with the given lines of text rendered at
/// `(x_mm, y_mm)` in Helvetica 12pt.
pub fn build_pdf(lines: &[(f32, f32, &str)]) -> Vec<u8> {
    let (doc, page1, layer1) = PdfDocument::new("test", Mm(210.0), Mm(297.0), "Layer 1");
    let layer = doc.get_page(page1).get_layer(layer1);
    let font = doc.add_builtin_font(BuiltinFont::Helvetica).unwrap();

    for &(x, y, text) in lines {
        layer.use_text(text, 12.0, Mm(x), Mm(y), &font);
    }

    doc.save_to_bytes().unwrap()
}
