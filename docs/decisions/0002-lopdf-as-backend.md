---
tags: [adr, accepted]
---

# ADR 0002 — `lopdf` as the parsing backend

**Status**: Accepted · **Date**: 2026-05-17

## Context

Given [[decisions/0001-pure-rust-vs-ffi|that we go pure Rust]], we had
to choose between the main pure-Rust crates:

- `lopdf` (J-F-Liu) — 2.2k stars, mature low-level parser.
- `pdf-rs` / `pdf` crate — 1.7k stars, separate PDF parser.
- `pdf-extract` (jrmuizel) — facade over lopdf with simple
  `extract_text`.
- `pdf_oxide` — new, weekly releases, already does something similar.

## Decision

We use **`lopdf 0.40`** as the single parsing dependency.

## Consequences

### What `lopdf` gives us

- Decoding of the header + xref + objects.
- `lopdf::content::Content::decode(bytes) -> Vec<Operation>`. **This
  saves us from implementing the PDF tokenizer**, which is tricky due
  to whitespace/comments/literal strings/hex strings.
- `Document::get_page_content(page_id) -> Vec<u8>` with FlateDecode
  already applied.
- `Document::get_page_fonts(page_id) -> BTreeMap<Vec<u8>, &Dictionary>`.
- `Dictionary::get_font_encoding(doc) -> Result<Encoding>` with cascade
  resolution: WinAnsi/MacRoman/Standard/Identity-H+ToUnicode.
- Internal ToUnicode CMap parser (not exposed but usable via
  `Encoding::UnicodeMapEncoding`).

### What it **doesn't** give us, and we cover

- **`/Encoding /Differences`** overrides. Covered by
  `src/parser/fonts/differences.rs`.
- **Type0 `/W` widths**. Covered by `src/parser/fonts/widths.rs`.
- **Adobe Glyph List lookup**. Subset in
  `src/parser/fonts/glyph_names.rs`.
- **Full content-stream state machine** with CTM, text matrix, text
  state. Covered by `src/parser/lopdf_backend.rs`.

### Why we keep `lopdf` **internal** (`pub(crate)`)

If tomorrow we want to swap to `pdf-rs`, `pdfium-render` (FFI feature
flag), or any other backend, we can do it without breaking the public
API. The public types (`Document`, `Page`, `Char`, `Word`) are
backend-agnostic.

### Trade-offs accepted

- `lopdf` does not publicly expose `ToUnicodeCMap` or `glyphnames`. We
  have to go through `Encoding` to decode and maintain our own glyph-
  name table.
- `lopdf`'s `default-features` drags `chrono`, `jiff`, `time`, `rayon`.
  We turn them off with `default-features = false`.

## Alternatives considered

### `pdf-rs` / `pdf` crate

- ➕ Higher-level API than lopdf, `PageRc`, `Resolve` trait.
- ➖ Writing API is experimental; the parser covers fewer font features
  in its stable public version. No clear advantage over lopdf.

### `pdf-extract`

- ➕ Already has a state machine over lopdf, returns plain text.
- ➖ Its API is `extract_text_from_mem() -> String`. No positions, no
  layout. **Internally it does have positions**, but doesn't expose
  them. We use it as **inspiration** but re-implement.

### `pdf_oxide`

- ➕ Weekly releases, excellent performance, already handles tables.
- ➖ Young crate (2026), API in flux. Starting on top of an immature
  crate puts risk on our own.

## Re-evaluation

If we hit complex embedded-font cases that lopdf cannot decode and our
own code path for `Differences`/`Type0` is not enough, we would
consider:

1. Contributing upstream to lopdf.
2. Adding `pdfium-render` as an optional feature flag (`feature = "ffi"`).

## Related reading

- [[decisions/0001-pure-rust-vs-ffi]] — pure Rust as the root decision
- [[architecture/state-machine]] — what we built on top of lopdf's API
- [[reference/font-encodings]] — the area where we lean on lopdf the most
