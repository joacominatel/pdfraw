//! `lopdf`-based PDF parser.
//!
//! Walks a page's content stream as a state machine, tracking the text
//! matrix, current font, and graphics state stack, and emits one
//! [`Char`] per glyph drawn by `Tj`/`TJ`/`'`/`"`.

use crate::char::Char;
use crate::error::{Error, Result};
use crate::geom::{Matrix, Point};
use crate::page::{Page, PageMetrics};
use compact_str::CompactString;
use lopdf::content::Content;
use lopdf::{Dictionary, Document as LDoc, Encoding, Object, ObjectId};
use std::collections::HashMap;

/// Read width/height/rotation for the page.
pub(crate) fn page_metrics(
    doc: &LDoc,
    page_id: ObjectId,
    page_index: usize,
) -> Result<PageMetrics> {
    let page = doc.get_dictionary(page_id)?;
    let media_box =
        resolve_inheritable(doc, page, b"MediaBox").ok_or_else(|| Error::ContentStream {
            page: page_index,
            reason: "missing /MediaBox".into(),
        })?;
    let (width, height) = media_box_dimensions(page_index, &media_box)?;
    let rotation = resolve_inheritable(doc, page, b"Rotate")
        .and_then(|o| match o {
            Object::Integer(i) => Some(i as i16),
            _ => None,
        })
        .unwrap_or(0);
    Ok(PageMetrics {
        width,
        height,
        rotation,
    })
}

/// Walk the page-tree `/Parent` chain looking for an inheritable key.
///
/// PDF defines `MediaBox`, `Rotate`, `Resources`, and `CropBox` as
/// inheritable from the closest ancestor `/Pages` node that defines them.
fn resolve_inheritable(doc: &LDoc, page: &Dictionary, key: &[u8]) -> Option<Object> {
    let mut cursor: &Dictionary = page;
    loop {
        if let Ok(v) = cursor.get(key) {
            return Some(deref(doc, v.clone()));
        }
        let Ok(Object::Reference(id)) = cursor.get(b"Parent") else {
            return None;
        };
        let Ok(parent) = doc.get_dictionary(*id) else {
            return None;
        };
        cursor = parent;
    }
}

fn deref(doc: &LDoc, obj: Object) -> Object {
    match obj {
        Object::Reference(id) => doc.get_object(id).cloned().unwrap_or(Object::Null),
        other => other,
    }
}

fn media_box_dimensions(page_index: usize, o: &Object) -> Result<(f32, f32)> {
    let arr = match o {
        Object::Array(a) => a,
        _ => {
            return Err(Error::ContentStream {
                page: page_index,
                reason: "/MediaBox is not an array".into(),
            });
        }
    };
    if arr.len() != 4 {
        return Err(Error::ContentStream {
            page: page_index,
            reason: format!("/MediaBox has {} elements, expected 4", arr.len()),
        });
    }
    let n = |o: &Object| -> Result<f32> {
        match o {
            Object::Integer(i) => Ok(*i as f32),
            Object::Real(r) => Ok(*r),
            _ => Err(Error::ContentStream {
                page: page_index,
                reason: "/MediaBox contains a non-numeric value".into(),
            }),
        }
    };
    let x0 = n(&arr[0])?;
    let y0 = n(&arr[1])?;
    let x1 = n(&arr[2])?;
    let y1 = n(&arr[3])?;
    Ok(((x1 - x0).abs(), (y1 - y0).abs()))
}

/// Extract every glyph on the page as a [`Char`].
pub(crate) fn extract_chars(page: &Page<'_>) -> Result<Vec<Char>> {
    let doc = &page.document().inner;
    let page_id = page.page_id();
    let page_height = page.height();

    let raw = doc
        .get_page_content(page_id)
        .map_err(|e| Error::ContentStream {
            page: page.index(),
            reason: format!("get_page_content: {e}"),
        })?;
    let content = Content::decode(&raw).map_err(|e| Error::ContentStream {
        page: page.index(),
        reason: format!("decode content: {e}"),
    })?;

    let mut fonts = FontTable::default();
    if let Ok(map) = doc.get_page_fonts(page_id) {
        for (name, dict) in map {
            fonts.insert(name, FontInfo::from_dict(doc, dict));
        }
    }

    let mut ts = TextState::default();
    let mut gs_stack: Vec<Matrix> = vec![Matrix::IDENTITY];
    let mut in_text = false;
    let mut out = Vec::new();

    for op in content.operations {
        match op.operator.as_str() {
            // graphics state
            "q" => gs_stack.push(*gs_stack.last().unwrap_or(&Matrix::IDENTITY)),
            "Q" => {
                gs_stack.pop();
                if gs_stack.is_empty() {
                    gs_stack.push(Matrix::IDENTITY);
                }
            }
            "cm" => {
                if let Some(m) = matrix_from_operands(&op.operands) {
                    if let Some(top) = gs_stack.last_mut() {
                        *top = m.then(*top);
                    }
                }
            }

            // text object
            "BT" => {
                in_text = true;
                ts.tm = Matrix::IDENTITY;
                ts.tlm = Matrix::IDENTITY;
            }
            "ET" => {
                in_text = false;
            }

            // text state
            "Tf" => {
                if let (Some(name), Some(size)) = (
                    op.operands.first().and_then(name_of),
                    op.operands.get(1).and_then(num_of),
                ) {
                    ts.font_name = name;
                    ts.font_size = size;
                }
            }
            "Tc" => ts.char_space = first_num(&op.operands).unwrap_or(0.0),
            "Tw" => ts.word_space = first_num(&op.operands).unwrap_or(0.0),
            "Tz" => ts.h_scale = first_num(&op.operands).unwrap_or(100.0) / 100.0,
            "TL" => ts.leading = first_num(&op.operands).unwrap_or(0.0),
            "Ts" => ts.rise = first_num(&op.operands).unwrap_or(0.0),
            "Tr" => { /* rendering mode ignored — we emit all glyphs */ }

            // text positioning
            "Tm" => {
                if let Some(m) = matrix_from_operands(&op.operands) {
                    ts.tm = m;
                    ts.tlm = m;
                }
            }
            "Td" => {
                if let (Some(tx), Some(ty)) = (
                    op.operands.first().and_then(num_of),
                    op.operands.get(1).and_then(num_of),
                ) {
                    let new = Matrix::translation(tx, ty).then(ts.tlm);
                    ts.tm = new;
                    ts.tlm = new;
                }
            }
            "TD" => {
                if let (Some(tx), Some(ty)) = (
                    op.operands.first().and_then(num_of),
                    op.operands.get(1).and_then(num_of),
                ) {
                    ts.leading = -ty;
                    let new = Matrix::translation(tx, ty).then(ts.tlm);
                    ts.tm = new;
                    ts.tlm = new;
                }
            }
            "T*" => {
                let new = Matrix::translation(0.0, -ts.leading).then(ts.tlm);
                ts.tm = new;
                ts.tlm = new;
            }

            // text showing
            "Tj" if in_text => {
                if let Some(bytes) = first_string(&op.operands) {
                    emit_string(&bytes, &mut ts, &gs_stack, &fonts, page_height, &mut out);
                }
            }
            "'" if in_text => {
                // next line + show
                let new = Matrix::translation(0.0, -ts.leading).then(ts.tlm);
                ts.tm = new;
                ts.tlm = new;
                if let Some(bytes) = first_string(&op.operands) {
                    emit_string(&bytes, &mut ts, &gs_stack, &fonts, page_height, &mut out);
                }
            }
            "\"" if in_text => {
                // operands: aw ac string
                if op.operands.len() == 3 {
                    ts.word_space = num_of(&op.operands[0]).unwrap_or(0.0);
                    ts.char_space = num_of(&op.operands[1]).unwrap_or(0.0);
                }
                let new = Matrix::translation(0.0, -ts.leading).then(ts.tlm);
                ts.tm = new;
                ts.tlm = new;
                if let Some(bytes) = op.operands.last().and_then(string_bytes) {
                    emit_string(&bytes, &mut ts, &gs_stack, &fonts, page_height, &mut out);
                }
            }
            "TJ" if in_text => {
                if let Some(Object::Array(arr)) = op.operands.first() {
                    for item in arr {
                        match item {
                            Object::String(bytes, _) => emit_string(
                                bytes,
                                &mut ts,
                                &gs_stack,
                                &fonts,
                                page_height,
                                &mut out,
                            ),
                            Object::Integer(i) => {
                                let tx = -(*i as f32 / 1000.0) * ts.font_size * ts.h_scale;
                                ts.tm = Matrix::translation(tx, 0.0).then(ts.tm);
                            }
                            Object::Real(r) => {
                                let tx = -(*r / 1000.0) * ts.font_size * ts.h_scale;
                                ts.tm = Matrix::translation(tx, 0.0).then(ts.tm);
                            }
                            _ => {}
                        }
                    }
                }
            }

            _ => { /* unsupported operator — silently skip operands */ }
        }
    }

    Ok(out)
}

// ============================================================================
// Helpers
// ============================================================================

fn matrix_from_operands(operands: &[Object]) -> Option<Matrix> {
    if operands.len() < 6 {
        return None;
    }
    let n = |i: usize| num_of(&operands[i]);
    Some(Matrix::new(n(0)?, n(1)?, n(2)?, n(3)?, n(4)?, n(5)?))
}

fn num_of(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

fn first_num(operands: &[Object]) -> Option<f32> {
    operands.first().and_then(num_of)
}

fn name_of(obj: &Object) -> Option<Vec<u8>> {
    match obj {
        Object::Name(n) => Some(n.clone()),
        _ => None,
    }
}

fn string_bytes(obj: &Object) -> Option<Vec<u8>> {
    match obj {
        Object::String(b, _) => Some(b.clone()),
        _ => None,
    }
}

fn first_string(operands: &[Object]) -> Option<Vec<u8>> {
    operands.first().and_then(string_bytes)
}

// ============================================================================
// Text state and fonts
// ============================================================================

#[derive(Debug, Clone)]
struct TextState {
    tm: Matrix,
    tlm: Matrix,
    font_name: Vec<u8>,
    font_size: f32,
    char_space: f32,
    word_space: f32,
    h_scale: f32,
    leading: f32,
    rise: f32,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            tm: Matrix::IDENTITY,
            tlm: Matrix::IDENTITY,
            font_name: Vec::new(),
            font_size: 1.0,
            char_space: 0.0,
            word_space: 0.0,
            h_scale: 1.0,
            leading: 0.0,
            rise: 0.0,
        }
    }
}

/// Per-page font cache: name (e.g. b"F1") -> decoded info.
#[derive(Default)]
struct FontTable<'doc> {
    by_name: HashMap<Vec<u8>, FontInfo<'doc>>,
}

impl<'doc> FontTable<'doc> {
    fn insert(&mut self, name: Vec<u8>, info: FontInfo<'doc>) {
        self.by_name.insert(name, info);
    }

    fn get(&self, name: &[u8]) -> Option<&FontInfo<'doc>> {
        self.by_name.get(name)
    }
}

struct FontInfo<'doc> {
    encoding: Option<Encoding<'doc>>,
    widths: Widths,
    is_composite: bool,
    /// `/Encoding /Differences` overrides for byte-code → Unicode.
    differences: Option<crate::parser::fonts::differences::Differences>,
}

impl FontInfo<'_> {
    /// Decode one glyph code into the Unicode string it produces. Returns
    /// `(text, advance_bytes)` so callers can iterate variable-length codes
    /// in composite fonts.
    fn decode(&self, bytes: &[u8]) -> String {
        // If we have a per-code override from /Differences, build the
        // decoded string ourselves so we can substitute on byte basis. We
        // only do this for simple fonts (1 byte per code).
        if let Some(diffs) = &self.differences {
            if !self.is_composite {
                let mut s = String::with_capacity(bytes.len());
                let base_decoded = match &self.encoding {
                    Some(enc) => enc.bytes_to_string(bytes).ok(),
                    None => None,
                };
                let base_chars: Option<Vec<char>> =
                    base_decoded.as_ref().map(|d| d.chars().collect());
                for (i, b) in bytes.iter().enumerate() {
                    if let Some(c) = diffs.get(b) {
                        s.push(*c);
                    } else if let Some(bc) = base_chars.as_ref().and_then(|v| v.get(i)) {
                        s.push(*bc);
                    } else {
                        s.push(*b as char);
                    }
                }
                return s;
            }
        }
        match &self.encoding {
            Some(enc) => enc.bytes_to_string(bytes).unwrap_or_else(|_| latin1(bytes)),
            None => latin1(bytes),
        }
    }
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Glyph-width table. PDF widths are stored in units of 1/1000 of the font
/// size; values here are pre-divided by 1000 (i.e. width = ratio of font
/// size). Missing entries fall back to `default_width`.
struct Widths {
    by_code: HashMap<u32, f32>,
    default_width: f32,
}

impl Widths {
    fn empty() -> Self {
        Self {
            by_code: HashMap::new(),
            default_width: 0.5,
        }
    }

    fn width_of(&self, code: u32) -> f32 {
        self.by_code
            .get(&code)
            .copied()
            .unwrap_or(self.default_width)
    }
}

impl<'doc> FontInfo<'doc> {
    fn from_dict(doc: &'doc LDoc, dict: &'doc Dictionary) -> Self {
        let encoding = dict.get_font_encoding(doc).ok();
        let is_composite = matches!(
            dict.get(b"Subtype").and_then(Object::as_name).ok(),
            Some(b"Type0")
        );
        let widths = if is_composite {
            crate::parser::fonts::widths::extract_type0(doc, dict)
                .map(|by_code| Widths {
                    by_code,
                    default_width: 0.5,
                })
                .unwrap_or_else(Widths::empty)
        } else {
            simple_widths(dict).unwrap_or_else(Widths::empty)
        };
        let differences = crate::parser::fonts::differences::extract(doc, dict);
        Self {
            encoding,
            widths,
            is_composite,
            differences,
        }
    }
}

fn simple_widths(dict: &Dictionary) -> Option<Widths> {
    let first_char = dict.get(b"FirstChar").ok().and_then(|o| match o {
        Object::Integer(i) => Some(*i as u32),
        _ => None,
    })?;
    let widths_arr = match dict.get(b"Widths").ok()? {
        Object::Array(a) => a,
        _ => return None,
    };
    let mut by_code = HashMap::with_capacity(widths_arr.len());
    for (i, w) in widths_arr.iter().enumerate() {
        if let Some(v) = num_of(w) {
            by_code.insert(first_char + i as u32, v / 1000.0);
        }
    }
    let default_width = dict
        .get(b"MissingWidth")
        .ok()
        .and_then(num_of)
        .map(|v| v / 1000.0)
        .unwrap_or(0.5);
    Some(Widths {
        by_code,
        default_width,
    })
}

// ============================================================================
// Glyph emission
// ============================================================================

fn emit_string(
    bytes: &[u8],
    ts: &mut TextState,
    gs_stack: &[Matrix],
    fonts: &FontTable,
    page_height: f32,
    out: &mut Vec<Char>,
) {
    let ctm = *gs_stack.last().unwrap_or(&Matrix::IDENTITY);
    let font = fonts.get(&ts.font_name);

    let decoded: String = font
        .map(|f| f.decode(bytes))
        .unwrap_or_else(|| latin1(bytes));

    let fontname_str = String::from_utf8_lossy(&ts.font_name).into_owned();
    let fontname = CompactString::from(&fontname_str);

    let is_composite = font.is_some_and(|f| f.is_composite);

    // Phase 1 simplification: we map decoded chars to byte positions by
    // walking bytes one-by-one for simple fonts and 2-by-2 for composite.
    // This is an approximation; multi-byte CMap ranges may shift things.
    let codes: Vec<u32> = if is_composite {
        bytes
            .chunks(2)
            .map(|c| match c {
                [a, b] => ((*a as u32) << 8) | *b as u32,
                [a] => *a as u32,
                _ => 0,
            })
            .collect()
    } else {
        bytes.iter().map(|&b| b as u32).collect()
    };

    // Iterate decoded chars in parallel with codes, using whichever is
    // shorter (defensive — they should match in length most of the time).
    for (code_idx, ch) in decoded.chars().enumerate() {
        let code = codes.get(code_idx).copied().unwrap_or(0);

        let w = font.map(|f| f.widths.width_of(code)).unwrap_or(0.5);
        let glyph_width = w * ts.font_size;

        // Position in text space is (0, rise); transform through tm then ctm.
        let trm = ts.tm.then(ctm);
        let origin = trm.transform(Point::new(0.0, ts.rise));
        let upper = trm.transform(Point::new(0.0, ts.rise + ts.font_size));

        let x_scale = trm.x_scale();
        let effective_size = trm.y_scale().max(x_scale);
        // Top-down conversion: PDF y grows upwards from page bottom.
        let top = page_height - upper.y;
        let bottom = page_height - origin.y;

        let glyph_render_width = glyph_width * x_scale;
        let x0 = origin.x;
        let x1 = x0 + glyph_render_width;

        let upright = trm.is_upright();

        if !ch.is_control() {
            out.push(Char {
                text: CompactString::from(ch.to_string().as_str()),
                x0,
                x1,
                top,
                bottom,
                doctop: top,
                size: effective_size,
                fontname: fontname.clone(),
                upright,
            });
        }

        // Advance text matrix in *text* space (before ctm).
        let adv_t = (w * ts.font_size + ts.char_space + word_space_for(ch, ts)) * ts.h_scale;
        ts.tm = Matrix::translation(adv_t, 0.0).then(ts.tm);
    }
}

fn word_space_for(ch: char, ts: &TextState) -> f32 {
    if ch == ' ' { ts.word_space } else { 0.0 }
}
