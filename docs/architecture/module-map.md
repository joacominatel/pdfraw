---
tags: [architecture, module-map]
---

# Module map

A quick tour of `src/` so you know where to look before you start
modifying anything.

```
src/
├── lib.rs              entry point, re-exports, #![deny(missing_docs)]
├── prelude.rs          convenience re-exports for users
├── document.rs         Document::open / from_bytes / page / pages
├── page.rs             Page<'doc> with lazy chars/metrics cache
├── char.rs             struct Char (a positioned glyph)
├── word.rs             struct Word + WordOptions
├── error.rs            enum Error (non_exhaustive) + Result alias
├── geom.rs             Matrix (2D affine), BBox, Point
│
├── text/
│   ├── mod.rs          subsystem re-exports
│   ├── options.rs      TextOptions + TextOptionsBuilder (fluent)
│   ├── cluster.rs      cluster_objects: greedy 1D
│   ├── extractor.rs    WordExtractor + LayoutEngine
│   ├── ligatures.rs    FB00–FB06 → "ff", "fi", "fl", …
│   └── textmap.rs      TextMap (output→source mapping)
│
└── parser/             ← pub(crate), internal
    ├── mod.rs
    ├── lopdf_backend.rs    PDF state machine (heart of the crate)
    ├── scan_detect.rs      image-only page detection
    └── fonts/
        ├── mod.rs
        ├── glyph_names.rs   Adobe glyph name → char
        ├── differences.rs   /Encoding /Differences parser
        └── widths.rs        Type0 /W array parser
```

## Public vs internal

| Category | Modules |
|-----------|---------|
| **Public** (stable, semver) | `document`, `page`, `char`, `word`, `error`, `geom`, `text`, `prelude` |
| **Internal** (`pub(crate)`) | `parser` and all its children |

The reasoning for keeping `parser` internal is in
[[decisions/0002-lopdf-as-backend]]: we want to swap the backend without
breaking the public API.

## File-by-file walkthrough

### `lib.rs`
Crate-level doc-comment with a quick start. Re-exports `Document`,
`Page`, `Char`, `Word`, `WordOptions`, `TextOptions`,
`TextOptionsBuilder`, `Error`, `Result`. Enables
`#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`.

### `document.rs`
- `Document` wraps `lopdf::Document` + `page_ids`.
- `from_lopdf` checks for encryption and returns `Error::Unsupported`.
- `Pages` is the page iterator (see [[decisions/0007-lazy-char-cache]]
  for why each page has its own cache).

### `page.rs`
- `Page<'doc>` with `OnceLock<Vec<Char>>` and `OnceLock<PageMetrics>`.
- `chars()` parses on demand and caches; on error it does not cache,
  allowing a retry.
- Public methods: `width`, `height`, `rotation`, `index`, `document`,
  `chars`, `words`, `extract_text`, `extract_text_layout`, `is_scanned`.

### `char.rs` / `word.rs`
Plain DTOs using `CompactString` to avoid allocations on short ASCII.
`WordOptions::default()` matches pdfplumber.

### `error.rs`
`Error` enum with `#[non_exhaustive]` + thiserror. Variants: `Io`, `Pdf`,
`ContentStream`, `Unsupported`, `Font`, `PageOutOfBounds`. See
[[decisions/0005-error-type-design]].

### `geom.rs`
`Matrix` 2D affine (6 coefficients). Methods: `then`, `transform`,
`pre_translate`, `x_scale`, `y_scale`, `is_upright`. Tests cover each
algebraic identity.

### `text/extractor.rs`
- `extract_words`: uses `Cow<'_, [Char]>` to avoid cloning when there is
  no ligature expansion ([[decisions/0006-cow-in-word-extraction]]).
- `extract_text_simple`: naive word-join.
- `extract_text_layout`: the algorithm from
  [[architecture/layout-algorithm]].

### `text/cluster.rs`
Generic greedy 1D clustering. ~50 LOC, 5 tests.

### `parser/lopdf_backend.rs`
**The densest file in the crate.** ~530 LOC with the full PDF state
machine. See [[architecture/state-machine]].

### `parser/fonts/*`
- `glyph_names.rs` — const subset table of the Adobe Glyph List (~150
  glyphs).
- `differences.rs` — parses `/Encoding /Differences [code /name …]`.
- `widths.rs` — parses Type0 `/W` arrays (both array and range forms).

### `parser/scan_detect.rs`
Heuristic: 0 `BT` operators + at least one `Do` to an XObject `/Image`
→ scanned.

## Conventions we follow in the code

- `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` at crate level.
- `thiserror` for errors, no `anyhow` (per Apollo Rust handbook
  chapter 4).
- `&[T]` and `&str` over `Vec<T>` and `String` in parameters.
- `?` operator over `match` chains.
- Tests named `subject_behavior_when_condition`.
- One assertion per test when possible.

More detail in [[testing/strategy]].
