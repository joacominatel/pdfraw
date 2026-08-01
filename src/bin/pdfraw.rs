//! Extrae texto de un PDF y lo guarda en un `.txt` con el mismo nombre.
use pdfraw::prelude::*;
use std::{fs, path::Path};

fn main() -> Result<()> {
    let path = std::env::args().nth(1).expect("uso: pdfraw <archivo.pdf>");
    let doc = Document::open(&path)?;
    let opts = TextOptions::pdfplumber_defaults();
    let mut out = String::new();

    for page in doc.pages() {
        let page = page?;
        if page.is_scanned()? {
            continue;
        }
        out.push_str(&page.extract_text_layout(&opts)?);
        out.push('\n');
    }

    let out_path = Path::new(&path).with_extension("txt");
    fs::write(&out_path, &out)?;
    eprintln!("→ {}", out_path.display());
    Ok(())
}
