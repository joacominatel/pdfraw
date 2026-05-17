---
tags: [reference, api]
---

# Reference: public API

Browsable summary of the crate's public surface. For the full doc,
`cargo doc --open --no-deps -p pdf_extractor`.

## Public modules

| Module | Re-exported in `prelude` | Purpose |
|--------|---------------------------|-----------|
| `pdf_extractor::document` | `Document` | Opening PDFs |
| `pdf_extractor::page` | `Page` | View of a single page |
| `pdf_extractor::char` | `Char` | Positioned glyph |
| `pdf_extractor::word` | `Word`, `WordOptions` | Char cluster |
| `pdf_extractor::text` | `TextOptions` | Extractor knobs |
| `pdf_extractor::error` | `Error`, `Result` | Error types |
| `pdf_extractor::geom` | — | `Matrix`, `BBox`, `Point` |
| `pdf_extractor::parser` | — | **`pub(crate)`**, not part of the API |

## `Document`

```rust
pub struct Document { /* private */ }

impl Document {
    pub fn open(path: impl AsRef<Path>) -> Result<Self>;
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self>;
    pub fn num_pages(&self) -> usize;
    pub fn page(&self, index: usize) -> Result<Page<'_>>;
    pub fn pages(&self) -> Pages<'_>;            // Iterator<Item=Result<Page<'_>>>
}
```

**Errors:**
- `Error::Io` — file not found / not readable (only `open`).
- `Error::Pdf` — bytes are not a valid PDF.
- `Error::Unsupported("encrypted PDF — …")` — PDF has `/Encrypt`.
- `Error::PageOutOfBounds(i)` — `page(i)` with `i >= num_pages()`.

## `Page<'doc>`

```rust
pub struct Page<'doc> { /* private */ }

impl<'doc> Page<'doc> {
    pub fn index(&self) -> usize;
    pub fn document(&self) -> &'doc Document;
    pub fn width(&self) -> f32;        // pt
    pub fn height(&self) -> f32;       // pt
    pub fn rotation(&self) -> i16;     // 0, 90, 180, 270

    pub fn chars(&self) -> Result<&[Char]>;
    pub fn words(&self, opts: &WordOptions) -> Result<Vec<Word>>;

    pub fn extract_text(&self) -> Result<String>;
    pub fn extract_text_layout(&self, opts: &TextOptions) -> Result<String>;

    pub fn is_scanned(&self) -> Result<bool>;
}
```

**Cache:** `chars()` and the derived methods (`words`,
`extract_text*`) share a lazy cache (`OnceLock<Vec<Char>>`). The first
call parses, the next ones are O(1). On error nothing is cached — a
retry tries again. Detail in [[decisions/0007-lazy-char-cache]].

**Errors:** every `Result<_>` above can return
`Error::ContentStream { page, reason }` if the stream is malformed, or
`Error::Pdf` if the dictionary lookup fails.

## `Char`

```rust
pub struct Char {
    pub text:     CompactString,
    pub x0:       f32,
    pub x1:       f32,
    pub top:      f32,    // top-down, just like pdfplumber
    pub bottom:   f32,
    pub doctop:   f32,    // cumulative top across pages
    pub size:     f32,
    pub fontname: CompactString,
    pub upright:  bool,
}
```

## `Word` and `WordOptions`

```rust
pub struct Word {
    pub text:       CompactString,
    pub x0, x1, top, bottom: f32,
    pub char_range: Range<usize>,
}

pub struct WordOptions {
    pub x_tolerance:        f32,    // default 3.0
    pub y_tolerance:        f32,    // default 3.0
    pub x_tolerance_ratio:  Option<f32>,
    pub keep_blank_chars:   bool,
    pub use_text_flow:      bool,
    pub expand_ligatures:   bool,
}

impl Default for WordOptions { /* pdfplumber defaults */ }
```

## `TextOptions` and `TextOptionsBuilder`

```rust
pub struct TextOptions {
    pub x_tolerance:        f32,    // 3.0
    pub y_tolerance:        f32,    // 3.0
    pub x_tolerance_ratio:  Option<f32>,
    pub x_density:          f32,    // 7.25
    pub y_density:          f32,    // 13.0
    pub keep_blank_chars:   bool,
    pub use_text_flow:      bool,
    pub expand_ligatures:   bool,
}

impl TextOptions {
    pub fn pdfplumber_defaults() -> Self;
    pub fn builder() -> TextOptionsBuilder;
}

impl TextOptionsBuilder {
    pub fn x_tolerance(self, v: f32) -> Self;
    pub fn y_tolerance(self, v: f32) -> Self;
    pub fn x_tolerance_ratio(self, v: Option<f32>) -> Self;
    pub fn x_density(self, v: f32) -> Self;
    pub fn y_density(self, v: f32) -> Self;
    pub fn keep_blank_chars(self, v: bool) -> Self;
    pub fn use_text_flow(self, v: bool) -> Self;
    pub fn expand_ligatures(self, v: bool) -> Self;
    pub fn build(self) -> TextOptions;
}
```

For what each one does: [[guides/tuning-text-options]].

## `Error`

```rust
#[non_exhaustive]
pub enum Error {
    Io(std::io::Error),
    Pdf(lopdf::Error),
    ContentStream { page: usize, reason: String },
    Unsupported(&'static str),
    Font(String),
    PageOutOfBounds(usize),
}

pub type Result<T> = std::result::Result<T, Error>;
```

Design details in [[decisions/0005-error-type-design]].

> **Important**: `Error` is `#[non_exhaustive]`. If you `match` on it,
> add a `_` arm; future variants are not breaking.

## `geom`

Low-level API exposed in case you need to manipulate raw matrices or
positions. You won't touch this in normal use.

```rust
pub struct Point { pub x: f32, pub y: f32 }
pub struct BBox  { pub x0: f32, pub top: f32, pub x1: f32, pub bottom: f32 }
pub struct Matrix {
    pub a: f32, pub b: f32, pub c: f32,
    pub d: f32, pub e: f32, pub f: f32,
}

impl Matrix {
    pub const IDENTITY: Matrix;
    pub fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self;
    pub fn translation(tx: f32, ty: f32) -> Self;
    pub fn scale(sx: f32, sy: f32) -> Self;
    pub fn then(self, other: Self) -> Self;
    pub fn transform(&self, p: Point) -> Point;
    pub fn pre_translate(self, tx: f32, ty: f32) -> Self;
    pub fn x_scale(&self) -> f32;
    pub fn y_scale(&self) -> f32;
    pub fn is_upright(&self) -> bool;
}
```

## Prelude

```rust
pub use crate::char::Char;
pub use crate::document::Document;
pub use crate::error::{Error, Result};
pub use crate::page::Page;
pub use crate::text::options::TextOptions;
pub use crate::word::{Word, WordOptions};
```

Typical use:

```rust
use pdf_extractor::prelude::*;
```

## Stability

- v0.1.x: API **unstable**, we may make breaking changes between
  patches.
- v0.2+: we commit to semver.

Future changes documented in [[roadmap]] and in each superseded ADR.
