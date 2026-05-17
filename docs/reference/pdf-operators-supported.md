---
tags: [reference, pdf-operators]
---

# Supported PDF operators

Full table of the PDF operators recognized by the state machine. Other
operators are silently ignored (their operands stay structured in
`Operation.operands`, so there is no parser-shift risk).

Source: `src/parser/lopdf_backend.rs::extract_chars`.

## Supported — text

| Operator | Operands | Meaning | Crate action |
|----------|-----------|-------------|------------------|
| `BT` | — | Begin Text | Enter text mode, reset `tm = tlm = identity` |
| `ET` | — | End Text | Leave text mode |
| `Tj` | `(str)` | Show text | Decode + emit positioned chars |
| `TJ` | `[arr]` | Show text array | For each item: string → emit; number → kern |
| `'` | `(str)` | Move to next line and show | `T*` + `Tj` |
| `"` | `aw ac (str)` | Set spacing, move, show | `Tw=aw`, `Tc=ac`, `T*`, `Tj` |

## Supported — text state

| Operator | Operands | Meaning | Action |
|----------|-----------|-------------|--------|
| `Tf` | `/name size` | Set font and size | Store `font_name` and `font_size` |
| `Tc` | `n` | Char spacing | Store in `text_state.char_space` |
| `Tw` | `n` | Word spacing | Store in `text_state.word_space` (only applies to byte 0x20) |
| `Tz` | `n` | Horizontal scale (%) | Store as `n/100` |
| `TL` | `n` | Leading | Store in `text_state.leading` |
| `Ts` | `n` | Text rise | Store in `text_state.rise` |
| `Tr` | `n` | Rendering mode | Tracked but ignored (we always emit) |

## Supported — text positioning

| Operator | Operands | Meaning | Action |
|----------|-----------|-------------|--------|
| `Tm` | `a b c d e f` | Set text matrix | `tm = tlm = Matrix(a,b,c,d,e,f)` |
| `Td` | `tx ty` | Translate text matrix | `tm = tlm = translate(tx,ty) ⨯ tlm` |
| `TD` | `tx ty` | Translate + set leading | Like `Td` but `leading = -ty` |
| `T*` | — | Move to next line | `tm = tlm = translate(0,-leading) ⨯ tlm` |

## Supported — graphics state

| Operator | Operands | Meaning | Action |
|----------|-----------|-------------|--------|
| `q` | — | Save graphics state | `gs_stack.push(top)` |
| `Q` | — | Restore graphics state | `gs_stack.pop()` |
| `cm` | `a b c d e f` | Concatenate to CTM | `CTM = Matrix(...) ⨯ CTM` |

## Supported — XObject reference

| Operator | Operands | Action |
|----------|-----------|--------|
| `Do` | `/name` | **Emits no chars**, but `scan_detect.rs` collects it to detect image-only pages |

## Ignored (skip operands)

The following operators are common in PDFs and silently skipped. Their
operands stay in `Operation.operands` and do not throw off the parser
(unlike a naive tokenizer where an operand could mis-align everything).

### Paths
`m`, `l`, `c`, `v`, `y`, `re`, `h`, `S`, `s`, `f`, `F`, `f*`, `B`, `B*`,
`b`, `b*`, `n`, `W`, `W*`

### Color
`G`, `g`, `RG`, `rg`, `K`, `k`, `CS`, `cs`, `SC`, `sc`, `SCN`, `scn`

### Line / dash / stroke
`w`, `J`, `j`, `M`, `d`, `ri`, `i`, `gs`

### Shading and inline images
`sh`, `BI`, `ID`, `EI`

### Marked content
`MP`, `DP`, `BMC`, `BDC`, `EMC`

### Compatibility
`BX`, `EX`

## The `Tr` operator (rendering mode)

Even though we store it, **we don't filter by mode**. PDF defines:

| `Tr` | Meaning |
|------|-------------|
| 0 | Fill |
| 1 | Stroke |
| 2 | Fill + stroke |
| 3 | Invisible (used for OCR text overlay) |
| 4–7 | Variants with clipping |

`Tr=3` matters for "invisible text" behind an OCR-overlay image. Right
now we emit it like the rest, meaning a scanned PDF with OCR overlay
will return text via `extract_text*` even though `is_scanned()` also
returns true. Edge case documented in [[roadmap]].

## How to add support for a new operator

1. Identify its arity and semantics in the PDF 1.7 specification.
2. Add a `match` arm in `extract_chars` (or `scan_detect`).
3. If it changes state, add it to `TextState` or `GraphicsState`.
4. Add a specific test in `tests/extraction.rs` with a PDF that uses it.

## Related reading

- [[architecture/state-machine]]
- PDF 1.7 Specification (ISO 32000-1) — the canonical source
