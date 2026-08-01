# pdfraw

A Rust library that pulls raw, layout-preserving text out of PDFs.
It's a port of [pdfplumber](https://github.com/jsvine/pdfplumber)'s
`extract_text(layout=True)` algorithm — same defaults, same output
shape, but written from scratch on top of [`lopdf`](https://crates.io/crates/lopdf)
in pure Rust. No FFI, no dylibs, no Python.

The use case it was built for is invoices and credit notes: documents
where the relationship between a line item and its right-aligned price
is the whole point. If you need that text to come out the other side
with the spatial relationships intact, this crate does that.

## Status

v0.1.1. The crate works and its test suite covers the parser, the
layout algorithm, font decoding, malformed-input handling, and
end-to-end extraction on synthetic PDFs. On a corpus of 40 real
invoices (~1.5 MB total) it pulls 99% of the content pdfplumber finds,
about 21 times faster.

0.1.1 is an audit release: seven defects fixed, including two that
changed the values of public `Char` fields (`size` and `doctop`) and
one that let a malformed PDF hang the parser. If you read either field,
read [`CHANGELOG.md`](CHANGELOG.md) before upgrading. I would not call
the API frozen yet — expect breaking changes between 0.1.x releases
until 0.2 lands.

## When to use it

- You need to extract text from invoices, receipts, statements, or
  similar documents where columns and right-alignment matter.
- You want to ship a single self-contained binary — no Python runtime,
  no shared libraries.
- You care about throughput. A typical invoice page takes under a
  millisecond.

## When not to use it

- The PDFs you process are scanned images. `pdfraw` tells you when a
  page is scanned but does not OCR it. Pair it with Tesseract or a
  cloud OCR service for those.
- You need rich table extraction with cell detection. That is planned
  for v0.2 but is not in this release.
- You need to handle password-encrypted PDFs. `Document::open` returns
  an error on those for now.
- You need to read forms, annotations, or signatures. Different scope.

## Install

```toml
[dependencies]
pdfraw = "0.1"
```

Requires Rust 1.88 or newer — that floor comes from `lopdf`, not from
anything this crate does.

## The command-line tool

The same crate ships a `pdfraw` binary. Library users never pay for it —
Cargo does not build a dependency's binaries.

```sh
pdfraw invoice.pdf                # text to stdout
pdfraw invoice.pdf -o out.txt     # text to a file
```

Pages with no text layer are skipped and reported on stderr with their
index, so a scanned page never silently vanishes from the output.

## A small example

```rust
use pdfraw::prelude::*;

fn main() -> Result<()> {
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
    Ok(())
}
```

The output preserves the original 2D layout:

```
ACME                                               Invoice


Line A                                                     1.00
Line B                                                     2.00


Total                                                      3.00
```

If you need lower-level access, `page.chars()` gives you every glyph
with its position, font, and size; `page.words(opts)` clusters those
glyphs into words; and `Char`/`Word`/`BBox` are public so you can do
your own layout analysis.

## Tuning

The defaults match pdfplumber's. If they don't fit your PDFs, the
builder lets you adjust them:

```rust
let opts = TextOptions::builder()
    .x_tolerance(1.5)        // tighter word grouping
    .y_density(11.0)         // denser vertical spacing in output
    .expand_ligatures(true)  // turn ﬁ into fi
    .build();
```

The full set of options and a "raise this when / lower this when"
cheatsheet lives in [`docs/guides/tuning-text-options.md`](docs/guides/tuning-text-options.md).

## Numbers

Benchmarked against pdfplumber 0.11 on a corpus of 40 real invoice PDFs
(78 pages, 191 KB of extracted text, total 1.4 MB on disk):

|                       | pdfraw     | pdfplumber | ratio |
|-----------------------|------------|------------|-------|
| Total wall time       | 122 ms     | 2.6 s      | 21x   |
| Median per-PDF        | 1.9 ms     | 30 ms      | 16x   |
| p95 per-PDF           | 6.4 ms     | n/a        |       |
| Throughput            | 1080 pg/s  | n/a        |       |
| Content recall        | 99%        | 100%       |       |
| PDFs below 95% recall | 0 of 40    | n/a        |       |

The 1% gap is mostly whitespace and ordering noise — pdfraw is a bit
more compact in how it pads columns. None of it is missing content.

To reproduce on your own corpus:

```sh
cargo run --release --example bench_dir -- /path/to/your/pdfs
```

See [`docs/testing/strategy.md`](docs/testing/strategy.md) for the
testing layout and [`docs/decisions/`](docs/decisions/) for the design
rationale.

## What's in the box

The public API surface is small:

- `Document::open(path)` / `Document::from_bytes(bytes)` — parse a PDF.
- `Document::pages()` / `Document::page(i)` — iterate pages.
- `Page::extract_text_layout(opts)` — the pdfplumber-style layout text.
- `Page::extract_text()` — plain words joined with spaces, lines with
  newlines. Faster, no layout.
- `Page::chars()` / `Page::words(opts)` — the underlying positioned
  primitives.
- `Page::is_scanned()` — heuristic that returns true when a page emits
  no text operators but draws images.
- `TextOptions` / `WordOptions` — the knobs.
- `Error` — a `#[non_exhaustive]` enum with `thiserror` derives; one
  variant per failure mode.

A short reference is at [`docs/reference/api.md`](docs/reference/api.md);
full rustdoc is `cargo doc --open --no-deps -p pdfraw`.

## What it gets right and what it gives up

It implements:

- Pure-Rust PDF parsing via `lopdf`.
- A content-stream state machine that handles `BT`/`ET`, `Tj`/`TJ`/`'`/`"`,
  the full text matrix, the graphics-state stack, char and word
  spacing, horizontal scaling, leading, rise.
- Font decoding for WinAnsi, MacRoman, StandardEncoding, MacExpert,
  PDFDocEncoding, Identity-H/V with `/ToUnicode` CMaps, custom
  `/Encoding /Differences` overrides, and Type0 (CID) widths.
- Char → Word → Line clustering with the same tolerance model as
  pdfplumber.
- Layout reconstruction over a virtual monospaced grid (defaults
  `x_density = 7.25 pt`, `y_density = 13.0 pt`).
- A heuristic that flags scanned/image-only pages so callers can route
  them to OCR.

It does not:

- Descend into Form XObjects (text inside a form is invisible to the
  extractor).
- Rotate or vertical text in the layout output. The chars come out
  flagged with `upright = false` but the layout reconstruction
  ignores them.
- Handle encrypted PDFs.
- Detect tables as structured objects.

If any of those break a real PDF for you, open an issue with the file
and what you expected.

## Acknowledgments

The algorithm is a port of pdfplumber. None of the analysis it does
would have been possible without [Jeremy Singer-Vine](https://github.com/jsvine)'s
work and the careful prior art at Tabula and in Anssi Nurminen's
thesis. PDF parsing rides on top of [`lopdf`](https://github.com/J-F-Liu/lopdf),
which does the heavy lifting of xref tables, FlateDecode, and content
stream tokenization.

## License

MIT. See [`LICENSE`](LICENSE).
