---
tags: [roadmap, planning]
---

# Roadmap

Prioritized view of what's missing. Each item may deserve an ADR before
being implemented — see conventions in [[decisions/index]].

## ✅ v0.1.1 (audit pass — done)

- [x] `Char.size` reports the real font size, not the matrix scale
- [x] `Char.doctop` accumulates preceding page heights
- [x] `q`/`Q` save and restore the text state (PDF 32000-1 Table 52)
- [x] Cyclic `/Parent` chains no longer hang the parser
- [x] Malformed `/W` CID ranges no longer exhaust memory
- [x] A bad glyph name no longer discards a whole `/Differences` map
- [x] `Word.char_range` is correct when glyphs arrive out of reading order
- [x] Dependencies current (`lopdf` 0.44, `compact_str` 0.10)
- [x] rustdoc builds with zero warnings

## ✅ v0.1 (MVP — done)

- [x] Document open + from_bytes
- [x] Page metrics (width/height/rotation)
- [x] Char extraction with top-down coordinates
- [x] WordExtractor (pdfplumber)
- [x] extract_text_layout (pdfplumber)
- [x] Font decoding: WinAnsi, MacRoman, Identity-H+ToUnicode,
      `/Differences`, Type0 widths
- [x] Adobe Glyph List subset
- [x] is_scanned() detection
- [x] Encrypted-PDF rejection (Error::Unsupported)
- [x] Tests: unit, integration, snapshot and doctest suites (run
      `cargo test --all-features` for the current count)
- [x] Documentation: full Obsidian vault

## 🚧 v0.2 (next)

### Tables (`extract_tables`)

Replicate `pdfplumber.extract_tables()` with strategies:
- `"lines"` — detect tables by drawn borders.
- `"text"` — infer tables by columnar word alignment.

ADR pending: algorithmic strategy. Tabula and Anssi Nurminen's thesis
are the reference.

### `Word.doctop` and cross-page navigation

Today `Word` does not carry `doctop` (`Char` does). For multi-page
invoices, that's useful. Trivial to add.

### Benches with `criterion`

Target: < 50 ms/page in release for typical PDFs (~2000 chars). Not
systematically measured today.

### Detect metadata

`Document::metadata() -> Metadata` with title, author, producer,
creation date, mod date. lopdf already exposes it.

## 🔮 v0.3+

### Opt-in OCR

`feature = "ocr"` with `tesseract-rs` or alternative.
`Page::ocr() -> Result<String>` for scanned pages. Decision via ADR
when it lands: native integration or caller callback.

### Password-encrypted PDFs

`Document::open_with_password(path, password)`. lopdf already supports
it internally.

### Cross-page parallelism

`feature = "parallel"` is already reserved. With `rayon`:

```rust
pub fn pages_parallel<F, R>(&self, f: F) -> Vec<Result<R>>
    where F: Fn(Page<'_>) -> Result<R> + Sync + Send,
          R: Send;
```

Each page has its own lazy cache (see
[[decisions/0007-lazy-char-cache]]), so there is no contention.

### Output formats

- `to_markdown()` — heading inference, code blocks via monospace fonts.
- `to_html()` — preserving positions with
  `<div style="position:absolute">`.
- `to_json()` with `feature = "serde"` already toggleable.

### Bounding-box queries

```rust
impl<'doc> Page<'doc> {
    pub fn chars_in_bbox(&self, bbox: BBox) -> Result<Vec<&Char>>;
    pub fn crop(&self, bbox: BBox) -> Result<CroppedPage<'_>>;
}
```

Useful for extracting "the total block" without parsing everything.

### Visual debugging

`feature = "debug-render"` that produces a PNG with char, word and
line bboxes overlaid on the PDF render. Analogous to
`pdfplumber.PDF.to_image()`.

## ❌ Out of scope

| Feature | Why not |
|---------|------------|
| PDF writing / editing | Other crate (lopdf already does it) |
| Form filling | Different use case |
| Digital signatures | Heavy, off our "extraction" focus |
| Rendering to image | Needs FFI (PDFium) |
| Adobe Reader plugin | Not our stack |

## How to contribute an item

1. Make sure it's in this roadmap. If not, open an issue to discuss.
2. If non-trivial, open an ADR (`docs/decisions/NNNN-title.md`).
3. Implementation + tests + docs in the same PR.
4. Update [[roadmap]] (check the box).

## Related reading

- [[decisions/0004-mvp-scope]] — what we left out of the MVP and why
