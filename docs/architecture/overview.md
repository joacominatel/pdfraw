---
tags: [architecture, overview]
---

# Architecture: the big picture

`pdf_extractor` is a **pure-Rust library** that turns a PDF into text
while preserving the original layout. The architecture is organized into
**4 sequential stages**, each with a clear responsibility that can be
tested in isolation.

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌──────────────────┐
│   PDF bytes     │ →  │  State machine  │ →  │  Word extractor │ →  │  Layout engine   │
│ (lopdf-parsed)  │    │   (Stage 1)     │    │   (Stage 2)     │    │   (Stage 3)      │
└─────────────────┘    └────────┬────────┘    └────────┬────────┘    └────────┬─────────┘
                                ↓                      ↓                      ↓
                          Vec<Char> with         Vec<Word> with          String with
                          x,y positions          per-line bbox           newlines/spaces
                                                                         aligned
```

A fourth interface, `Page::is_scanned()`, reuses stage 1 (walks the same
content stream) but produces a `bool` instead of chars.

## Stages in detail

### Stage 0 — base PDF parsing

[`lopdf`](https://crates.io/crates/lopdf) does the heavy lifting:

- Decodes the xref header and object structure.
- Decompresses content streams (FlateDecode, etc.).
- Resolves indirect references.
- Provides access to the font dictionary via `get_page_fonts`.
- Implements the native PDF encoding decoders (`WinAnsi`, `MacRoman`,
  `Identity-H + ToUnicode`).

See [[decisions/0002-lopdf-as-backend]] for the rationale.

### Stage 1 — content stream → `Vec<Char>`

`parser::lopdf_backend::extract_chars` walks the list of
`lopdf::content::Operation` as a **state machine** (see
[[architecture/state-machine]]). For every `Tj`/`TJ`/`'`/`"`:

1. Decodes the bytes using the current font's encoding.
2. Computes each glyph's position by applying `tm * ctm`.
3. Flips Y to top-down.
4. Emits a `Char` with `x0, x1, top, bottom, size, fontname, upright`.

### Stage 2 — `Char[]` → `Word[]`

`text::extractor::extract_words` runs pdfplumber's `WordExtractor`:

1. 1D cluster by `top` with `y_tolerance` → lines.
2. Sort each line by `x0`.
3. Walk with the rule "new word on backtrack, gap > x_tolerance, or line
   break".

Detail in [[architecture/layout-algorithm]].

### Stage 3 — `Word[]` → layout `String`

`text::extractor::extract_text_layout` builds a **virtual monospaced
grid**: it divides the chars bbox by `x_density × y_density` and pads
with newlines and spaces so each word lands in its virtual column.

This is what keeps an invoice's right-aligned total looking right-
aligned in the text output.

## Type diagram

```
Document  owns ─→  lopdf::Document
   │
   │ borrows
   ↓
Page<'doc>  ─→ OnceLock<Vec<Char>> (lazy cache)
   │
   │ chars()
   ↓
&[Char]  ─→ extract_words()  ─→ Vec<Word>
   │                              │
   │ extract_text_layout()        │
   ↓                              ↓
String                       (also consumed by TextMap)
```

## Crate dependencies

| Dep | What it does |
|-----|----------|
| `lopdf` 0.40 | PDF parsing, content streams, fonts, native encodings |
| `thiserror` 2 | `Error` enum derive |
| `compact_str` 0.9 | Inline `CompactString` for `Char.text` / `Char.fontname` |
| `encoding_rs` 0.8 | Reserved for decoders not provided by lopdf |
| `log` 0.4 | Warnings for unmapped glyphs, ignored operators |

See `Cargo.toml` for exact versions. Rationale for each choice in
[[decisions/index]].

## What is **not** in this crate

- **OCR** — scanned PDFs are detected with `Page::is_scanned()` but OCR
  is out of scope (`feature = "ocr"` is a reserved placeholder for the
  future). See [[decisions/0004-mvp-scope]].
- **Table detection** — pdfplumber has `extract_tables()`; we don't, at
  least not in v0.1.
- **Encryption** — `Document::open` returns `Error::Unsupported` when it
  detects `/Encrypt`.
- **PDF writing/editing** — `lopdf` supports it but we only read.

## Related reading

- [[architecture/module-map]] — file-by-file tour
- [[architecture/state-machine]] — the heart of stage 1
- [[architecture/layout-algorithm]] — the algorithm of stage 3
- [[decisions/index]] — why each piece is the way it is
