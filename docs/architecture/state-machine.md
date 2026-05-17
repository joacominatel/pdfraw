---
tags: [architecture, state-machine, parser]
---

# PDF state machine (stage 1)

> File: `src/parser/lopdf_backend.rs::extract_chars`
> ~530 LOC. The heart of the crate.

PDF draws text by interpreting a binary **content stream** as a sequence
of operators. To extract positioned glyphs, we walk those operators
while maintaining our own state.

## Inputs and output

```rust
fn extract_chars(page: &Page<'_>) -> Result<Vec<Char>>
```

Internally:
1. `doc.get_page_content(page_id)` → `Vec<u8>` (lopdf decompresses).
2. `Content::decode(raw)` → `Vec<Operation>` (lopdf tokenizes).
3. `doc.get_page_fonts(page_id)` → table `b"F1" → &Dictionary`.
4. Our walk emits `Vec<Char>`.

## State we maintain

```rust
struct GraphicsState { ctm: Matrix /* ...via stack */ }
struct TextState {
    tm:          Matrix,       // text matrix
    tlm:         Matrix,       // text line matrix (remembers line start)
    font_name:   Vec<u8>,      // "F1"
    font_size:   f32,
    char_space:  f32,          // Tc
    word_space:  f32,          // Tw
    h_scale:     f32,          // Tz / 100
    leading:     f32,          // TL
    rise:        f32,          // Ts
}
```

Plus: `gs_stack: Vec<Matrix>` (for `q`/`Q`) and `in_text: bool` (to
know whether we are between `BT`/`ET`).

## Supported operators

### Graphics state
- `q` — push CTM.
- `Q` — pop CTM.
- `cm a b c d e f` — `CTM = matrix(a,b,c,d,e,f) ⨯ CTM`.

### Text object
- `BT` — enter text mode, reset `tm = tlm = identity`.
- `ET` — leave text mode.

### Text state
- `Tf /F1 12` — set font + size.
- `Tc x` — char spacing.
- `Tw x` — word spacing (only applies to byte `0x20`).
- `Tz x` — horizontal scale, as percent.
- `TL x` — leading (line-to-line distance).
- `Ts x` — text rise (super/subscript offset).
- `Tr x` — rendering mode (ignored, we always emit).

### Text positioning
- `Tm a b c d e f` — set `tm = tlm = matrix(...)`.
- `Td tx ty` — `tm = tlm = translate(tx,ty) ⨯ tlm`.
- `TD tx ty` — like `Td` but also `leading = -ty`.
- `T*` — `tm = tlm = translate(0, -leading) ⨯ tlm`.

### Text showing
- `Tj "string"` — paint string.
- `'"` (apostrophe) — next line + paint string.
- `" aw ac string"` — next line + `Tw=aw, Tc=ac` + paint.
- `TJ [array]` — paint an array mixing strings and numeric kerning.

### Ignored operators
Any operator not listed is silently skipped (its operands already come
structured in `Operation.operands`, so there is no risk of a parser
shift). The most common ones we ignore:

- Paths (`m`, `l`, `c`, `re`, `S`, `f`, `B`, …)
- Color (`g`, `G`, `rg`, `RG`, `sc`, `SC`, …)
- Shading, clipping, marked content.
- Inline images (`BI`/`ID`/`EI`).
- `Do` (XObject) — relevant for [[#scanned-detection]] but ignored on
  the text path.

## How a glyph's position is computed

Inside `emit_string(bytes, ...)`:

```rust
let trm = ts.tm.then(ctm);                       // text rendering matrix
let origin = trm.transform(Point::new(0.0, ts.rise));
let upper  = trm.transform(Point::new(0.0, ts.rise + ts.font_size));

let top    = page_height - upper.y;              // flip to top-down
let bottom = page_height - origin.y;

let glyph_width = font.widths.width_of(code) * ts.font_size;
let x0 = origin.x;
let x1 = x0 + glyph_width * trm.x_scale();

let upright = trm.is_upright();
```

Then it emits the `Char` and advances the text matrix:

```rust
let adv = (glyph_width + ts.char_space + word_space_if_space) * ts.h_scale;
ts.tm = Matrix::translation(adv, 0.0).then(ts.tm);
```

## How we handle `TJ` (kerning)

`TJ` receives an array like `[(Hello) -100 (World)]`:

```rust
for item in arr {
    match item {
        Object::String(bytes, _) => emit_string(bytes, ...),
        Object::Integer(i) => {
            let tx = -(i as f32 / 1000.0) * ts.font_size * ts.h_scale;
            ts.tm = Matrix::translation(tx, 0.0).then(ts.tm);
        }
        Object::Real(r)    => { /* same */ }
        _ => {}
    }
}
```

A negative number pushes the following glyphs further apart (positive
kern); a positive one brings them closer.

## How we decode bytes to Unicode

Each glyph code is decoded using the current font's encoding:

```
bytes → FontInfo.decode():
    1. Is there an /Encoding /Differences? If yes, byte-by-byte override.
    2. If not, lopdf::Encoding::bytes_to_string() handles:
       - OneByteEncoding (WinAnsi, MacRoman, Standard, MacExpert)
       - UnicodeMapEncoding (Identity-H/V + ToUnicode CMap)
       - Others (rare)
    3. Fallback: Latin-1
```

More detail in [[reference/font-encodings]].

## Scanned detection

`parser::scan_detect::is_scanned` walks the **same** content stream but
with trivial logic:

```rust
for op in content.operations {
    match op.operator.as_str() {
        "BT" | "Tj" | "TJ" | "'" | "\"" => has_text = true,
        "Do" => image_refs.push(name_operand),
        _ => {}
    }
}
```

If there was no text **and** some `Do` resolves to an XObject
`/Subtype /Image`, the page is probably scanned and the API returns
`Ok(true)`.

## Known edge cases

- **Rotated / vertical text**: `Char.upright == false`. `extract_words`
  and `extract_text_layout` do **not** filter on upright yet (identical
  to pdfplumber). If the rotation is slight the grid still works.
- **Type0 without `/W`**: we fall back to 0.5em per glyph, which hurts
  the `x1` computation but not the `x0` (which comes from the text
  matrix).
- **Operators with variable arity** (e.g. `BI`/`ID`/`EI` for inline
  images): lopdf hands them to us as tokens; we ignore them one by one.

## Related reading

- [[architecture/layout-algorithm]] — what happens to the chars next
- [[reference/pdf-operators-supported]] — full operator table
- [[reference/font-encodings]] — the decoding sub-stage
- [[decisions/0002-lopdf-as-backend]] — why lopdf and not manual parsing
