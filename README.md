# pdf_extractor

Pure-Rust, layout-preserving PDF text extraction — a port of
[pdfplumber](https://github.com/jsvine/pdfplumber)'s `extract_text(layout=True)`
algorithm. Built for invoices, credit notes, statements, and other
documents where column alignment matters.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

## What it does

Given a PDF, it returns text that preserves the original 2D layout:
left/right alignment, vertical spacing between sections, and the right-
aligned numbers typical of invoices.

```
ACME                                               Invoice


Line A                                                     1.00
Line B                                                     2.00


Total                                                      3.00
```

The library exposes the underlying primitives too: positioned [`Char`]s,
clustered [`Word`]s, page metadata, and detection of scanned (image-only)
pages.

## Documentation

There's a full Obsidian-compatible vault in [`docs/`](docs/README.md) with
architecture, decisions (ADRs), guides, reference, and roadmap. Open the
folder as an Obsidian vault to navigate wikilinks; or just read it as
plain Markdown.

Quick links:
- [docs/guides/quick-start.md](docs/guides/quick-start.md)
- [docs/architecture/overview.md](docs/architecture/overview.md)
- [docs/decisions/index.md](docs/decisions/index.md)
- [docs/reference/api.md](docs/reference/api.md)

## Status

MVP. Implements:

- Pure-Rust PDF parsing on top of [`lopdf`](https://crates.io/crates/lopdf).
- Content-stream state machine: `BT`/`ET`, `Tj`/`TJ`/`'`/`"`, full text
  matrix and graphics-state stack, char/word spacing, horizontal scaling.
- Font decoding: WinAnsi / MacRoman / Standard / Identity-H + ToUnicode,
  plus `/Encoding /Differences` overrides and Type0 (CID) widths.
- Char → Word → Line clustering, configurable tolerances.
- pdfplumber-faithful layout reconstruction (`x_density = 7.25`,
  `y_density = 13.0`).
- Heuristic detection of image-only (scanned) pages.

Out of scope for now: OCR, table extraction, encryption, vertical/rotated
text in the layout output.

## Quick start

```rust
use pdf_extractor::prelude::*;

# fn main() -> Result<()> {
let doc = Document::open("invoice.pdf")?;
for page in doc.pages() {
    let page = page?;
    if page.is_scanned()? {
        eprintln!("page {} is scanned — needs OCR", page.index());
        continue;
    }
    let text = page.extract_text_layout(&TextOptions::pdfplumber_defaults())?;
    println!("--- page {} ---\n{text}", page.index());
}
# Ok(()) }
```

Or get the raw positioned glyphs:

```rust
use pdf_extractor::prelude::*;
# fn run(doc: &Document) -> Result<()> {
let page = doc.page(0)?;
for c in page.chars()? {
    println!("{} at ({}, {}) size={}", c.text, c.x0, c.top, c.size);
}
# Ok(()) }
```

## Tuning the layout

`TextOptions::builder()` lets you override pdfplumber's defaults:

```rust
use pdf_extractor::TextOptions;
let opts = TextOptions::builder()
    .x_tolerance(1.5)         // tighter word grouping
    .y_density(11.0)          // denser line spacing in output
    .expand_ligatures(true)   // ﬁ → fi
    .build();
```

| Option              | Default | Purpose                                          |
|---------------------|---------|--------------------------------------------------|
| `x_tolerance`       | 3.0 pt  | Max gap between glyphs of the same word          |
| `y_tolerance`       | 3.0 pt  | Max vertical diff for glyphs on the same line    |
| `x_tolerance_ratio` | None    | If set, `x_tol = ratio * font_size` (dynamic)    |
| `x_density`         | 7.25 pt | Virtual column width in the output grid          |
| `y_density`         | 13.0 pt | Virtual line height in the output grid           |
| `keep_blank_chars`  | false   | Keep literal spaces from the PDF stream          |
| `use_text_flow`     | false   | Preserve stream order instead of sorting         |
| `expand_ligatures`  | false   | Replace `ﬁ`/`ﬂ`/`ﬃ`/… with their letter forms    |

## Architecture

```
src/
  document.rs          Document::open, from_bytes, pages()
  page.rs              Page<'doc>, extract_text*, is_scanned, chars(), words()
  char.rs / word.rs    Positioned types
  text/
    options.rs         TextOptions + builder
    cluster.rs         Greedy 1D clustering
    extractor.rs       Word extraction + layout reconstruction
    ligatures.rs       FB00–FB06 expansion
    textmap.rs         Output-to-source char map
  parser/
    lopdf_backend.rs   Content-stream state machine
    fonts/
      glyph_names.rs   Adobe glyph name → Unicode
      differences.rs   /Encoding /Differences parser
      widths.rs        Type0 /W array parser
    scan_detect.rs     Image-only page heuristic
```

## Testing

```sh
cargo test                  # unit + integration + doc tests
cargo insta review          # update layout snapshots
cargo clippy --all-targets  # zero warnings on a clean tree
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE),
at your option.
