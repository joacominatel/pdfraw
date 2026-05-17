//! Parse a font dictionary's `/Encoding /Differences` override.
//!
//! In PDF, a font's encoding can be specified as a dictionary of the form
//! `<< /BaseEncoding /WinAnsiEncoding /Differences [32 /space 169 /copyright ...] >>`.
//! The differences array reassigns glyphs at specific byte codes.

use crate::parser::fonts::glyph_names;
use lopdf::{Dictionary, Document as LDoc, Object};
use std::collections::HashMap;

/// Override map: byte code → Unicode character.
pub type Differences = HashMap<u8, char>;

/// Extract the differences map from a font dictionary, if any.
pub fn extract(doc: &LDoc, font_dict: &Dictionary) -> Option<Differences> {
    let enc = font_dict.get(b"Encoding").ok()?;
    let enc = match enc {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    let enc_dict = match enc {
        Object::Dictionary(d) => d,
        _ => return None,
    };
    let diffs = match enc_dict.get(b"Differences").ok()? {
        Object::Array(a) => a,
        _ => return None,
    };

    let mut out = HashMap::new();
    let mut code: i64 = 0;
    for item in diffs {
        match item {
            Object::Integer(i) => code = *i,
            Object::Name(name) => {
                let s = std::str::from_utf8(name).ok()?;
                if let Some(c) = glyph_names::lookup(s) {
                    if (0..=255).contains(&code) {
                        out.insert(code as u8, c);
                    }
                }
                code += 1;
            }
            _ => {}
        }
    }

    if out.is_empty() { None } else { Some(out) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::Object;

    fn diffs_dict(arr: Vec<Object>) -> Dictionary {
        let mut enc = Dictionary::new();
        enc.set(b"Differences".to_vec(), Object::Array(arr));
        let mut font = Dictionary::new();
        font.set(b"Encoding".to_vec(), Object::Dictionary(enc));
        font
    }

    #[test]
    fn extracts_single_override() {
        let doc = lopdf::Document::new();
        let font = diffs_dict(vec![
            Object::Integer(169),
            Object::Name(b"copyright".to_vec()),
        ]);
        let d = extract(&doc, &font).unwrap();
        assert_eq!(d.get(&169), Some(&'©'));
    }

    #[test]
    fn sequential_codes_increment_implicitly() {
        let doc = lopdf::Document::new();
        let font = diffs_dict(vec![
            Object::Integer(32),
            Object::Name(b"space".to_vec()),
            Object::Name(b"exclam".to_vec()),
        ]);
        let d = extract(&doc, &font).unwrap();
        assert_eq!(d.get(&32), Some(&' '));
        assert_eq!(d.get(&33), Some(&'!'));
    }

    #[test]
    fn missing_encoding_returns_none() {
        let doc = lopdf::Document::new();
        let font = Dictionary::new();
        assert!(extract(&doc, &font).is_none());
    }
}
