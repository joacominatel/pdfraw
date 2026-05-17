//! Print the layout-preserving text for every page of a PDF.
//!
//! Usage:
//!     cargo run --example extract -- path/to/invoice.pdf

use pdf_extractor::prelude::*;

fn main() -> Result<()> {
    let path = std::env::args().nth(1).expect("usage: extract <path.pdf>");
    let doc = Document::open(&path)?;
    let opts = TextOptions::pdfplumber_defaults();

    for page in doc.pages() {
        let page = page?;
        println!("===== page {} =====", page.index());
        if page.is_scanned()? {
            println!("(scanned — no text layer)");
            continue;
        }
        println!("{}", page.extract_text_layout(&opts)?);
    }
    Ok(())
}
