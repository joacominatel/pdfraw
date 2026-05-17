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

/// Build a two-page PDF with separate lines per page.
pub fn build_two_page_pdf(page1_lines: &[(f32, f32, &str)], page2_lines: &[(f32, f32, &str)]) -> Vec<u8> {
    let (doc, p1, l1) = PdfDocument::new("two", Mm(210.0), Mm(297.0), "Layer 1");
    let font = doc.add_builtin_font(BuiltinFont::Helvetica).unwrap();
    let layer1 = doc.get_page(p1).get_layer(l1);
    for &(x, y, text) in page1_lines {
        layer1.use_text(text, 12.0, Mm(x), Mm(y), &font);
    }
    let (p2, l2) = doc.add_page(Mm(210.0), Mm(297.0), "Layer 2");
    let layer2 = doc.get_page(p2).get_layer(l2);
    for &(x, y, text) in page2_lines {
        layer2.use_text(text, 12.0, Mm(x), Mm(y), &font);
    }
    doc.save_to_bytes().unwrap()
}
