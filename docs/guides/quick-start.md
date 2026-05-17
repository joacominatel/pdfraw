---
tags: [guide, quick-start]
---

# Quick start

This guide takes you from zero to "I just extracted layout-preserving
text from a PDF" in five minutes.

## Installation

In your `Cargo.toml`:

```toml
[dependencies]
pdfraw = { git = "https://github.com/joacominatel/pdfraw" }
```

The crate is not on crates.io yet.

## First example

```rust
use pdfraw::prelude::*;

fn main() -> Result<()> {
    let doc = Document::open("invoice.pdf")?;
    let page = doc.page(0)?;
    let text = page.extract_text_layout(&TextOptions::pdfplumber_defaults())?;
    println!("{text}");
    Ok(())
}
```

What `extract_text_layout` returns looks like this:

```
ACME Corp                                Invoice #001

Item A                                                  $10.00
Item B                                                  $20.00

Total                                                   $30.00
```

Notice how "Invoice #001" is right-aligned and the prices share a
virtual column — that is part of the output, not a display artifact.

## Iterate every page

```rust
use pdfraw::prelude::*;

fn main() -> Result<()> {
    let doc = Document::open("contract.pdf")?;
    for page in doc.pages() {
        let page = page?;
        println!("--- page {} ---", page.index());
        println!("{}", page.extract_text_layout(&TextOptions::pdfplumber_defaults())?);
    }
    Ok(())
}
```

## Plain text (no layout)

If you only want everything concatenated:

```rust
let text = page.extract_text()?;
```

Words separated by `' '`, lines by `'\n'`. You lose horizontal
alignment.

## Access the raw glyphs

For custom analysis, iterate `Char`s with positions:

```rust
for c in page.chars()? {
    println!("{} @ ({:.1}, {:.1}) size={:.1}", c.text, c.x0, c.top, c.size);
}
```

Or cluster words with your own options:

```rust
let opts = WordOptions {
    x_tolerance: 2.0,
    y_tolerance: 4.0,
    ..WordOptions::default()
};
for w in page.words(&opts)? {
    println!("{} at x=[{:.1}..{:.1}]", w.text, w.x0, w.x1);
}
```

## Detect scanned PDFs

Before attempting extraction:

```rust
if page.is_scanned()? {
    eprintln!("page {} is image-only — needs OCR", page.index());
    continue;
}
```

More detail in [[reference/api]] and [[guides/extracting-invoices]].

## Common errors

| Error | What it means | What to do |
|-------|-----------|-----------|
| `Error::Io(_)` | File does not exist or can't be read | Validate the path |
| `Error::Pdf(_)` | Not a valid PDF | Check magic bytes `%PDF-` |
| `Error::Unsupported("encrypted PDF — ...")` | Password-protected | Wait for `Document::open_with_password` (see [[roadmap]]) |
| `Error::ContentStream { page, reason }` | Malformed content stream on that page | Report an issue with the PDF if Acrobat opens it fine |
| `Error::PageOutOfBounds(i)` | `i >= num_pages()` | Check with `num_pages()` first |

## What's next?

- Real invoice patterns: [[guides/extracting-invoices]]
- Tune the algorithm: [[guides/tuning-text-options]]
- Understand what happens inside: [[architecture/overview]]
