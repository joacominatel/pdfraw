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

/// Read the media box and rotation for the page.
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
    let media = media_box_dimensions(doc, page_index, &media_box)?;
    let rotation = resolve_inheritable(doc, page, b"Rotate")
        .and_then(|o| match o {
            Object::Integer(i) => Some(i),
            Object::Real(r) => Some(r as i64),
            _ => None,
        })
        .map(normalize_rotation)
        .unwrap_or(0);
    Ok(PageMetrics {
        media_width: media.width,
        media_height: media.height,
        origin_x: media.origin_x,
        origin_y: media.origin_y,
        rotation,
    })
}

/// Fold an arbitrary `/Rotate` into the `{0, 90, 180, 270}` set that
/// [`PageMetrics::rotation`] promises.
///
/// PDF requires `/Rotate` to be a multiple of 90, and permits negatives.
/// Files get both wrong, so negatives wrap into range and anything that is
/// not a quarter turn is discarded rather than passed through.
fn normalize_rotation(raw: i64) -> i16 {
    let wrapped = raw.rem_euclid(360);
    if wrapped % 90 != 0 {
        log::warn!("/Rotate {raw} is not a multiple of 90 — treating the page as unrotated");
        return 0;
    }
    wrapped as i16
}

/// Hard cap on how far up the `/Parent` chain we walk.
///
/// A well-formed page tree is a handful of levels deep. A malformed (or
/// hostile) PDF can point two `/Pages` nodes at each other, which would
/// otherwise spin forever, so we stop rather than trust the file.
const MAX_PAGE_TREE_DEPTH: usize = 64;

/// Walk the page-tree `/Parent` chain looking for an inheritable key.
///
/// PDF defines `MediaBox`, `Rotate`, `Resources`, and `CropBox` as
/// inheritable from the closest ancestor `/Pages` node that defines them.
///
/// Returns `None` if the key is absent, the chain breaks, or the chain is
/// longer than [`MAX_PAGE_TREE_DEPTH`] (which a cycle always is).
fn resolve_inheritable(doc: &LDoc, page: &Dictionary, key: &[u8]) -> Option<Object> {
    let mut cursor: &Dictionary = page;
    for _ in 0..MAX_PAGE_TREE_DEPTH {
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
    log::warn!("/Parent chain exceeded {MAX_PAGE_TREE_DEPTH} levels — treating as cyclic");
    None
}

fn deref(doc: &LDoc, obj: Object) -> Object {
    match obj {
        Object::Reference(id) => doc.get_object(id).cloned().unwrap_or(Object::Null),
        other => other,
    }
}

/// Follow one level of indirection without cloning.
///
/// [`deref`] hands back an owned `Object`, which is fine for the small
/// numbers it was written for but useless for resource dictionaries: those
/// have to outlive the borrow so a [`FontTable`] can hold references into
/// them.
fn deref_ref<'a>(doc: &'a LDoc, obj: &'a Object) -> Option<&'a Object> {
    match obj {
        Object::Reference(id) => doc.get_object(*id).ok(),
        other => Some(other),
    }
}

/// Borrowing counterpart to [`resolve_inheritable`], for keys whose value the
/// caller needs to keep referring to.
fn inheritable_ref<'a>(doc: &'a LDoc, page: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    let mut cursor: &'a Dictionary = page;
    for _ in 0..MAX_PAGE_TREE_DEPTH {
        if let Ok(v) = cursor.get(key) {
            return deref_ref(doc, v);
        }
        let Ok(Object::Reference(id)) = cursor.get(b"Parent") else {
            return None;
        };
        let Ok(parent) = doc.get_dictionary(*id) else {
            return None;
        };
        cursor = parent;
    }
    log::warn!("/Parent chain exceeded {MAX_PAGE_TREE_DEPTH} levels — treating as cyclic");
    None
}

/// Look a sub-dictionary up inside a resource dictionary, resolving a
/// reference if that is how the file stores it.
fn sub_dictionary<'a>(
    doc: &'a LDoc,
    resources: &'a Dictionary,
    key: &[u8],
) -> Option<&'a Dictionary> {
    let entry = resources.get(key).ok()?;
    deref_ref(doc, entry)?.as_dict().ok()
}

fn media_box_dimensions(doc: &LDoc, page_index: usize, o: &Object) -> Result<MediaBox> {
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
    // Any of the four may be an indirect reference. Reading them literally
    // failed the whole lookup, and the caller then silently fell back to a
    // default page size — so every glyph on the page came out mispositioned.
    let n = |o: &Object| -> Result<f32> {
        match deref(doc, o.clone()) {
            Object::Integer(i) => Ok(i as f32),
            Object::Real(r) => Ok(r),
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
    let (width, height) = ((x1 - x0).abs(), (y1 - y0).abs());

    // A degenerate box would give a 0-wide page and negative tops. A *missing*
    // /MediaBox already falls back to a usable default, so a useless one
    // should not be treated as more trustworthy than no box at all.
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(Error::ContentStream {
            page: page_index,
            reason: format!("/MediaBox has no usable area ({width} x {height})"),
        });
    }
    // The lower-left corner is the page's origin, and it is not always
    // (0, 0). Dropping it shifted every glyph by that corner, which could
    // even put an on-page glyph at a negative `top`.
    Ok(MediaBox {
        origin_x: x0.min(x1),
        origin_y: y0.min(y1),
        width,
        height,
    })
}

/// A page's `/MediaBox`, split into where it starts and how big it is.
struct MediaBox {
    origin_x: f32,
    origin_y: f32,
    width: f32,
    height: f32,
}

/// How deep `Do` may nest before we stop trusting the file.
///
/// Real documents nest a form or two. A cycle is caught separately by
/// [`Interp::active`], but a file can also nest legitimately-distinct forms
/// thousands deep, which would exhaust the stack instead of looping.
const MAX_XOBJECT_DEPTH: usize = 16;

/// Everything about the page that stays fixed while its content runs.
struct Ctx<'a> {
    doc: &'a LDoc,
    geom: PageGeometry,
    /// Media-box origin and `/Rotate`, folded in as the outermost transform.
    page_rotation: Matrix,
}

/// Interpreter state that a nested `Do` must share with its caller.
struct Interp {
    gs_stack: Vec<GraphicsState>,
    out: Vec<Char>,
    /// Form XObjects currently being executed, innermost last. A form that
    /// names one of these is asking us to recurse forever.
    active: Vec<ObjectId>,
}

/// Extract every glyph on the page as a [`Char`].
pub(crate) fn extract_chars(page: &Page<'_>) -> Result<Vec<Char>> {
    let doc = &page.document().inner;
    let page_id = page.page_id();
    let metrics = page.metrics();
    let ctx = Ctx {
        doc,
        geom: PageGeometry {
            height: metrics.display_height(),
            doctop_offset: page.document().doctop_offset(page.index()),
        },
        page_rotation: metrics.page_transform(),
    };

    let raw = doc.get_page_content(page_id);
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

    // Needed for `/XObject` lookup, and inheritable like `/MediaBox` is.
    let resources = doc
        .get_dictionary(page_id)
        .ok()
        .and_then(|p| inheritable_ref(doc, p, b"Resources"))
        .and_then(|o| o.as_dict().ok());

    let mut interp = Interp {
        gs_stack: vec![GraphicsState::default()],
        out: Vec::new(),
        active: Vec::new(),
    };
    run_content(&ctx, &content, &fonts, resources, &mut interp, 0);
    Ok(interp.out)
}

/// Execute one content stream: the page's own, or a Form XObject's.
///
/// `resources` is the dictionary that `Do` resolves XObject names against,
/// and `fonts` the table `Tf` resolves against. A form supplies its own of
/// each when it has them and inherits the caller's when it does not.
fn run_content(
    ctx: &Ctx<'_>,
    content: &Content,
    fonts: &FontTable<'_>,
    resources: Option<&Dictionary>,
    interp: &mut Interp,
    depth: usize,
) {
    let geom = &ctx.geom;
    let page_rotation = ctx.page_rotation;

    // The text matrices are per content stream: `BT` resets them, and a form
    // starts a fresh one rather than inheriting the caller's position.
    let mut tm = TextMatrices::default();

    // The stack is never allowed to empty (see the "Q" arm), so these two
    // helpers can always hand back a live graphics state. They read through
    // `interp` rather than holding a borrow of it, so a nested `Do` can pass
    // the whole interpreter down.
    macro_rules! gs {
        () => {
            interp
                .gs_stack
                .last()
                .expect("graphics stack is never empty")
        };
    }
    macro_rules! gs_mut {
        () => {
            interp
                .gs_stack
                .last_mut()
                .expect("graphics stack is never empty")
        };
    }

    for op in &content.operations {
        match op.operator.as_str() {
            // graphics state — q/Q save and restore the CTM *and* the text
            // state parameters (PDF 32000-1, Table 52).
            "q" => {
                let top = gs!().clone();
                interp.gs_stack.push(top);
            }
            "Q" => {
                interp.gs_stack.pop();
                if interp.gs_stack.is_empty() {
                    interp.gs_stack.push(GraphicsState::default());
                }
            }
            "cm" => {
                if let Some(m) = matrix_from_operands(&op.operands) {
                    let top = gs_mut!();
                    top.ctm = m.then(top.ctm);
                }
            }

            // Text object. `BT` resets the text matrices; `ET` carries no
            // state of its own now that showing text is not gated on being
            // inside a text object.
            "BT" => tm = TextMatrices::default(),
            "ET" => {}

            // text state
            "Tf" => {
                if let (Some(name), Some(size)) = (
                    op.operands.first().and_then(name_of),
                    op.operands.get(1).and_then(num_of),
                ) {
                    let t = &mut gs_mut!().text;
                    t.font_name = name;
                    t.font_size = size;
                }
            }
            "Tc" => gs_mut!().text.char_space = first_num(&op.operands).unwrap_or(0.0),
            "Tw" => gs_mut!().text.word_space = first_num(&op.operands).unwrap_or(0.0),
            "Tz" => gs_mut!().text.h_scale = first_num(&op.operands).unwrap_or(100.0) / 100.0,
            "TL" => gs_mut!().text.leading = first_num(&op.operands).unwrap_or(0.0),
            "Ts" => gs_mut!().text.rise = first_num(&op.operands).unwrap_or(0.0),
            "Tr" => { /* rendering mode ignored — we emit all glyphs */ }

            // text positioning
            "Tm" => {
                if let Some(m) = matrix_from_operands(&op.operands) {
                    tm.set_line(m);
                }
            }
            "Td" => {
                if let (Some(tx), Some(ty)) = (
                    op.operands.first().and_then(num_of),
                    op.operands.get(1).and_then(num_of),
                ) {
                    tm.next_line(tx, ty);
                }
            }
            "TD" => {
                if let (Some(tx), Some(ty)) = (
                    op.operands.first().and_then(num_of),
                    op.operands.get(1).and_then(num_of),
                ) {
                    gs_mut!().text.leading = -ty;
                    tm.next_line(tx, ty);
                }
            }
            "T*" => {
                let leading = gs!().text.leading;
                tm.next_line(0.0, -leading);
            }

            // Text showing.
            //
            // Deliberately not gated on being inside a BT/ET pair. The spec
            // says a show operator belongs to a text object, but files in the
            // wild omit the BT or close it early, and every reader that
            // matters still draws that text. Refusing to would lose the whole
            // page silently — and `is_scanned` would not flag it either,
            // because it does count the Tj.
            "Tj" => {
                if let Some(bytes) = first_string(&op.operands) {
                    emit_string(
                        &bytes,
                        &mut tm,
                        gs!(),
                        fonts,
                        geom,
                        page_rotation,
                        &mut interp.out,
                    );
                }
            }
            "'" => {
                // next line + show
                let leading = gs!().text.leading;
                tm.next_line(0.0, -leading);
                if let Some(bytes) = first_string(&op.operands) {
                    emit_string(
                        &bytes,
                        &mut tm,
                        gs!(),
                        fonts,
                        geom,
                        page_rotation,
                        &mut interp.out,
                    );
                }
            }
            "\"" => {
                // operands: aw ac string
                if op.operands.len() == 3 {
                    let t = &mut gs_mut!().text;
                    t.word_space = num_of(&op.operands[0]).unwrap_or(0.0);
                    t.char_space = num_of(&op.operands[1]).unwrap_or(0.0);
                }
                let leading = gs!().text.leading;
                tm.next_line(0.0, -leading);
                if let Some(bytes) = op.operands.last().and_then(string_bytes) {
                    emit_string(
                        &bytes,
                        &mut tm,
                        gs!(),
                        fonts,
                        geom,
                        page_rotation,
                        &mut interp.out,
                    );
                }
            }
            "TJ" => {
                if let Some(Object::Array(arr)) = op.operands.first() {
                    for item in arr {
                        match item {
                            Object::String(bytes, _) => emit_string(
                                bytes,
                                &mut tm,
                                gs!(),
                                fonts,
                                geom,
                                page_rotation,
                                &mut interp.out,
                            ),
                            Object::Integer(_) | Object::Real(_) => {
                                let t = &gs!().text;
                                let adj = num_of(item).unwrap_or(0.0);
                                let tx = -(adj / 1000.0) * t.font_size * t.h_scale;
                                tm.advance(tx);
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Draw an XObject. For a Form this means executing its content
            // stream inline; text drawn there is text on the page, and every
            // reader shows it.
            "Do" => {
                let Some(name) = op.operands.first().and_then(name_of) else {
                    continue;
                };
                run_form(ctx, &name, fonts, resources, interp, depth);
            }

            _ => { /* unsupported operator — silently skip operands */ }
        }
    }
}

/// Resolve `name` in the current `/XObject` resources and, if it is a form,
/// execute it.
///
/// Anything that does not check out — a missing name, an `/Image`, a broken
/// stream — is skipped in silence, exactly as an unsupported operator is. The
/// page's own text must not be lost because one of its forms is malformed.
fn run_form(
    ctx: &Ctx<'_>,
    name: &[u8],
    fonts: &FontTable<'_>,
    resources: Option<&Dictionary>,
    interp: &mut Interp,
    depth: usize,
) {
    let doc = ctx.doc;
    let Some(entry) = resources.and_then(|r| sub_dictionary(doc, r, b"XObject")) else {
        return;
    };
    let Ok(named) = entry.get(name) else {
        return;
    };
    // Kept before resolving, because the id is what identifies a form for
    // cycle detection. A form stored inline cannot be recursive anyway.
    let id = match named {
        Object::Reference(id) => Some(*id),
        _ => None,
    };
    let Some(stream) = deref_ref(doc, named).and_then(|o| o.as_stream().ok()) else {
        return;
    };
    // An /Image XObject's bytes are samples, not operators.
    if !matches!(
        stream.dict.get(b"Subtype").and_then(Object::as_name).ok(),
        Some(b"Form")
    ) {
        return;
    }

    if depth >= MAX_XOBJECT_DEPTH {
        log::warn!(
            "/{} nests Form XObjects more than {MAX_XOBJECT_DEPTH} deep — not descending further",
            String::from_utf8_lossy(name)
        );
        return;
    }
    if let Some(id) = id
        && interp.active.contains(&id)
    {
        log::warn!(
            "Form XObject /{} invokes itself — skipping the recursive call",
            String::from_utf8_lossy(name)
        );
        return;
    }

    let Ok(bytes) = stream.decompressed_content() else {
        return;
    };
    let Ok(content) = Content::decode(&bytes) else {
        log::warn!(
            "Form XObject /{} has an undecodable content stream — skipping it",
            String::from_utf8_lossy(name)
        );
        return;
    };

    // A form supplies its own resources when it has them; otherwise the
    // invoking stream's stay in scope (PDF 32000-1, §8.10.1).
    let own_resources = stream
        .dict
        .get(b"Resources")
        .ok()
        .and_then(|o| deref_ref(doc, o))
        .and_then(|o| o.as_dict().ok());
    let form_fonts = own_resources.map(|r| fonts_from_resources(doc, r));

    // `Do` behaves as if the form were bracketed by q … Q, with /Matrix
    // concatenated onto the CTM. Without the bracket the matrix would leak
    // into whatever the page draws next.
    let mut saved = interp
        .gs_stack
        .last()
        .expect("graphics stack is never empty")
        .clone();
    if let Some(m) = stream.dict.get(b"Matrix").ok().and_then(|o| match o {
        Object::Array(a) => matrix_from_operands(a),
        _ => None,
    }) {
        saved.ctm = m.then(saved.ctm);
    }
    interp.gs_stack.push(saved);
    if let Some(id) = id {
        interp.active.push(id);
    }

    run_content(
        ctx,
        &content,
        form_fonts.as_ref().unwrap_or(fonts),
        own_resources.or(resources),
        interp,
        depth + 1,
    );

    if id.is_some() {
        interp.active.pop();
    }
    interp.gs_stack.pop();
    if interp.gs_stack.is_empty() {
        interp.gs_stack.push(GraphicsState::default());
    }
}

/// Build a font table from a resource dictionary's `/Font` entry.
///
/// The page-level table comes from `lopdf`'s `get_page_fonts`, which also
/// walks the page tree for inherited resources. A form's resources are not
/// inheritable, so this reads the one dictionary and stops.
fn fonts_from_resources<'doc>(doc: &'doc LDoc, resources: &'doc Dictionary) -> FontTable<'doc> {
    let mut table = FontTable::default();
    let Some(font_dict) = sub_dictionary(doc, resources, b"Font") else {
        return table;
    };
    for (name, value) in font_dict.iter() {
        if let Some(dict) = deref_ref(doc, value).and_then(|o| o.as_dict().ok()) {
            table.insert(name.clone(), FontInfo::from_dict(doc, dict));
        }
    }
    table
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

/// Where the page sits, so glyph coordinates can be flipped to top-down and
/// offset into document space.
struct PageGeometry {
    /// Page height in points, used to flip PDF's bottom-up y axis.
    height: f32,
    /// Sum of the heights of every preceding page, for `Char::doctop`.
    doctop_offset: f32,
}

/// The text-state parameters that PDF stores in the graphics state and that
/// `q`/`Q` therefore save and restore (PDF 32000-1, Table 52).
///
/// Note that `Tm`/`Tlm` are deliberately *not* here: the text matrices are
/// reset by `BT` and are not part of the saved graphics state.
#[derive(Debug, Clone)]
struct TextParams {
    font_name: Vec<u8>,
    font_size: f32,
    char_space: f32,
    word_space: f32,
    h_scale: f32,
    leading: f32,
    rise: f32,
}

impl Default for TextParams {
    fn default() -> Self {
        Self {
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

/// One entry of the `q`/`Q` stack.
#[derive(Debug, Clone, Default)]
struct GraphicsState {
    ctm: Matrix,
    text: TextParams,
}

/// The text matrix and text line matrix, which live outside the graphics
/// state and are reset by every `BT`.
#[derive(Debug, Clone, Copy)]
struct TextMatrices {
    tm: Matrix,
    tlm: Matrix,
}

impl Default for TextMatrices {
    fn default() -> Self {
        Self {
            tm: Matrix::IDENTITY,
            tlm: Matrix::IDENTITY,
        }
    }
}

impl TextMatrices {
    /// `Tm`: replace both matrices.
    fn set_line(&mut self, m: Matrix) {
        self.tm = m;
        self.tlm = m;
    }

    /// `Td`/`TD`/`T*`/`'`: move to a new line relative to the line matrix.
    fn next_line(&mut self, tx: f32, ty: f32) {
        let new = Matrix::translation(tx, ty).then(self.tlm);
        self.set_line(new);
    }

    /// Advance the text matrix horizontally within the current line.
    fn advance(&mut self, tx: f32) {
        self.tm = Matrix::translation(tx, 0.0).then(self.tm);
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

/// One glyph code from a shown string, with the text it decodes to.
///
/// The text is a `String`, not a `char`, because a `/ToUnicode` entry may map
/// one code to several characters (`<0041>` → `"fi"`), and it may be empty
/// when the encoding defines no glyph for the code.
struct DecodedCode {
    code: u32,
    text: String,
}

impl FontInfo<'_> {
    /// Split a shown string into its glyph codes, decoding each one on its own.
    ///
    /// Decoding the whole string at once and then zipping the result against
    /// the code list assumed the two lined up index for index. They do not:
    /// an encoding that defines no glyph for a code yields a shorter string,
    /// and a one-to-many `/ToUnicode` entry yields a longer one. Either way
    /// every following glyph took the wrong code — and so the wrong width and
    /// the wrong advance. Decoding per code removes the assumption entirely.
    fn decode_codes(&self, bytes: &[u8]) -> Vec<DecodedCode> {
        // Composite fonts are read two bytes at a time. This is still an
        // approximation — a real CMap may define variable-length ranges — but
        // it is now applied consistently to codes *and* text.
        let chunk = if self.is_composite { 2 } else { 1 };
        bytes
            .chunks(chunk)
            .map(|c| {
                let code = c.iter().fold(0u32, |acc, &b| (acc << 8) | b as u32);
                DecodedCode {
                    code,
                    text: self.decode_one(c, code),
                }
            })
            .collect()
    }

    /// Decode a single code's bytes into the text it produces.
    fn decode_one(&self, bytes: &[u8], code: u32) -> String {
        // A /Differences override wins, but only for simple fonts: the array
        // is indexed by single byte codes.
        if !self.is_composite
            && let Some(diffs) = &self.differences
            && let Ok(byte) = u8::try_from(code)
            && let Some(c) = diffs.get(&byte)
        {
            return c.to_string();
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
    // A simple font is indexed by single bytes, so /FirstChar outside 0..=255
    // is malformed. `as u32` would have turned -1 into u32::MAX and then
    // overflowed the addition below (a debug panic, a silent wrap in release).
    let first_char = dict.get(b"FirstChar").ok().and_then(|o| match o {
        Object::Integer(i) => u8::try_from(*i).ok().map(u32::from),
        _ => None,
    })?;
    let widths_arr = match dict.get(b"Widths").ok()? {
        Object::Array(a) => a,
        _ => return None,
    };
    let mut by_code = HashMap::with_capacity(widths_arr.len());
    for (i, w) in widths_arr.iter().enumerate() {
        let Some(code) = u32::try_from(i)
            .ok()
            .and_then(|i| first_char.checked_add(i))
        else {
            break;
        };
        if let Some(v) = num_of(w) {
            by_code.insert(code, v / 1000.0);
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
    tm: &mut TextMatrices,
    gs: &GraphicsState,
    fonts: &FontTable,
    geom: &PageGeometry,
    page_rotation: Matrix,
    out: &mut Vec<Char>,
) {
    let ctm = gs.ctm.then(page_rotation);
    let ts = &gs.text;
    let font = fonts.get(&ts.font_name);

    let fontname_str = String::from_utf8_lossy(&ts.font_name).into_owned();
    let fontname = CompactString::from(&fontname_str);

    let is_composite = font.is_some_and(|f| f.is_composite);

    // One entry per glyph code, each carrying its own decoded text. Codes and
    // text can no longer drift apart, however the encoding behaves.
    let decoded: Vec<DecodedCode> = match font {
        Some(f) => f.decode_codes(bytes),
        None => bytes
            .iter()
            .map(|&b| DecodedCode {
                code: b as u32,
                text: (b as char).to_string(),
            })
            .collect(),
    };

    for DecodedCode { code, text } in decoded {
        let w = font.map(|f| f.widths.width_of(code)).unwrap_or(0.5);
        let glyph_width = w * ts.font_size;

        // Position in text space is (0, rise); transform through tm then ctm.
        let trm = tm.tm.then(ctm);
        let origin = trm.transform(Point::new(0.0, ts.rise));
        let upper = trm.transform(Point::new(0.0, ts.rise + ts.font_size));

        let x_scale = trm.x_scale();
        // `Tf` sets the size in text space; the matrices then scale it onto
        // the page. Reporting only the matrix scale would say "0.75" for a
        // 12pt font drawn under a 0.75 CTM.
        let effective_size = ts.font_size * trm.y_scale().max(x_scale);
        // Top-down conversion: PDF y grows upwards from page bottom.
        let top = geom.height - upper.y;
        let bottom = geom.height - origin.y;

        // `Tz` scales glyphs horizontally. It lives in the text state rather
        // than in `tm`, so it has to be applied to the box explicitly — the
        // advance below already carries it, which is why the two disagreed:
        // with `200 Tz` advances doubled while reported widths did not move.
        let glyph_render_width = glyph_width * ts.h_scale * x_scale;
        let x0 = origin.x;
        let x1 = x0 + glyph_render_width;

        let upright = trm.is_upright();

        // A negative font size (or a mirroring matrix) runs the box backwards.
        // `Char` documents x0 <= x1 and top <= bottom, and `build_word` folds
        // these with min/max, so hand back an ordered box either way.
        let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        let (top, bottom) = if top <= bottom {
            (top, bottom)
        } else {
            (bottom, top)
        };

        // A code the encoding defines no glyph for still occupies a position
        // and still advances — it just draws nothing, so no Char is emitted.
        // A code that decodes to several characters produces one Char holding
        // them all, which is what `Char::text` already documents, and advances
        // once rather than once per character.
        let visible: String = text.chars().filter(|c| !c.is_control()).collect();
        if !visible.is_empty() {
            out.push(Char {
                text: CompactString::from(visible.as_str()),
                x0,
                x1,
                top,
                bottom,
                doctop: top + geom.doctop_offset,
                size: effective_size,
                fontname: fontname.clone(),
                upright,
            });
        }

        // Advance text matrix in *text* space (before ctm).
        let word_space = word_space_for(code, is_composite, ts);
        let adv_t = (w * ts.font_size + ts.char_space + word_space) * ts.h_scale;
        tm.advance(adv_t);
    }
}

/// Word spacing per PDF 32000-1 §9.3.3: it applies to the single-byte code
/// 32, whatever that byte happens to decode to.
///
/// Keying it on the decoded character instead meant a font whose
/// `/Differences` remaps code 32 to some other glyph never received `Tw` at
/// all. The mirror case matters too: in a composite font a two-byte CID may
/// contain `0x20` without being a word space, and must not receive it.
fn word_space_for(code: u32, is_composite: bool, ts: &TextParams) -> f32 {
    // Composite fonts only get word spacing for a genuine single-byte 32,
    // which `codes` never produces from a two-byte CID.
    if is_composite {
        return 0.0;
    }
    if code == 32 { ts.word_space } else { 0.0 }
}
