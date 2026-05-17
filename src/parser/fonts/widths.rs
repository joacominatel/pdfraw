//! Parse Type0 (CID) font widths from a `/W` array.
//!
//! Format: `[ c1 [w1 w2 ...] | c1 c2 w | ... ]` where the first form gives
//! widths to successive CIDs starting at `c1`, and the second form sets
//! `[c1, c2]` all to `w`.

use lopdf::{Dictionary, Document as LDoc, Object};
use std::collections::HashMap;

/// Extract widths from a Type0 font dict's `DescendantFonts[0].W`.
///
/// Returns a map of CID → width (in 1/1000 of font size).
pub fn extract_type0(doc: &LDoc, font_dict: &Dictionary) -> Option<HashMap<u32, f32>> {
    // /Type0 fonts have an array of DescendantFonts (always 1 entry).
    let desc = font_dict.get(b"DescendantFonts").ok()?;
    let desc = match desc {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    let arr = match desc {
        Object::Array(a) => a,
        _ => return None,
    };
    let first = arr.first()?;
    let cid_dict = match first {
        Object::Reference(id) => doc.get_dictionary(*id).ok()?,
        Object::Dictionary(d) => d,
        _ => return None,
    };

    let w_arr = match cid_dict.get(b"W").ok()? {
        Object::Array(a) => a,
        _ => return None,
    };

    let default_width = cid_dict
        .get(b"DW")
        .ok()
        .and_then(|o| match o {
            Object::Integer(i) => Some(*i as f32),
            Object::Real(r) => Some(*r),
            _ => None,
        })
        .unwrap_or(1000.0)
        / 1000.0;
    let _ = default_width; // surfaced to caller separately if needed later

    let mut out = HashMap::new();
    let mut i = 0;
    while i < w_arr.len() {
        let first_cid = match num_of(&w_arr[i]) {
            Some(v) => v as u32,
            None => {
                i += 1;
                continue;
            }
        };
        i += 1;
        if i >= w_arr.len() {
            break;
        }
        match &w_arr[i] {
            Object::Array(widths) => {
                for (j, w) in widths.iter().enumerate() {
                    if let Some(v) = num_of(w) {
                        out.insert(first_cid + j as u32, v / 1000.0);
                    }
                }
                i += 1;
            }
            Object::Integer(_) | Object::Real(_) => {
                let last_cid = match num_of(&w_arr[i]) {
                    Some(v) => v as u32,
                    None => break,
                };
                i += 1;
                if i >= w_arr.len() {
                    break;
                }
                let w = match num_of(&w_arr[i]) {
                    Some(v) => v / 1000.0,
                    None => break,
                };
                i += 1;
                for cid in first_cid..=last_cid {
                    out.insert(cid, w);
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    if out.is_empty() { None } else { Some(out) }
}

fn num_of(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_font(w: Vec<Object>) -> (LDoc, Dictionary) {
        let mut doc = LDoc::new();
        let mut cid_dict = Dictionary::new();
        cid_dict.set(b"W".to_vec(), Object::Array(w));
        let cid_id = doc.add_object(Object::Dictionary(cid_dict));
        let mut font = Dictionary::new();
        font.set(b"DescendantFonts".to_vec(), Object::Array(vec![Object::Reference(cid_id)]));
        (doc, font)
    }

    #[test]
    fn array_form_indexed_widths() {
        let (doc, font) = build_font(vec![
            Object::Integer(10),
            Object::Array(vec![Object::Integer(500), Object::Integer(600)]),
        ]);
        let w = extract_type0(&doc, &font).unwrap();
        assert_eq!(w.get(&10), Some(&0.5));
        assert_eq!(w.get(&11), Some(&0.6));
    }

    #[test]
    fn range_form_shared_width() {
        let (doc, font) = build_font(vec![
            Object::Integer(20),
            Object::Integer(22),
            Object::Integer(700),
        ]);
        let w = extract_type0(&doc, &font).unwrap();
        assert_eq!(w.get(&20), Some(&0.7));
        assert_eq!(w.get(&22), Some(&0.7));
    }
}
