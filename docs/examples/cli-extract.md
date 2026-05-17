---
tags: [example, cli]
---

# Example: `extract` CLI

A minimal binary that extracts layout-preserving text from every page
of a PDF. The source lives at `examples/extract.rs`.

## Source

```rust
//! Print the layout-preserving text for every page of a PDF.
//!
//! Usage:
//!     cargo run --example extract -- path/to/invoice.pdf

use pdf_extractor::prelude::*;

fn main() -> Result<()> {
    let path = std::env::args().nth(1).expect("usage: extract <path.pdf>");
    let doc = Document::open(&path)?;
    let opts = TextOptions::pdfplumber_defaults();

    for page in doc.pages() {
        let page = page?;
        println!("===== page {} =====", page.index());
        if page.is_scanned()? {
            println!("(scanned — no text layer)");
            continue;
        }
        println!("{}", page.extract_text_layout(&opts)?);
    }
    Ok(())
}
```

## How to run it

```bash
cargo run --example extract -- /path/to/invoice.pdf
```

Expected output for a typical invoice:

```
===== page 0 =====
ACME Corp                                Invoice #001

Item A                                                  $10.00
Item B                                                  $20.00

Total                                                   $30.00
```

## Useful variations

### Stream to stdout for use with pipes

```rust
use std::io::Write;

let stdout = std::io::stdout();
let mut out = stdout.lock();
writeln!(out, "{text}").ok();
```

Combine with `| grep`, `| awk`, etc.

### With logging for debugging

`Cargo.toml`:
```toml
[dev-dependencies]
env_logger = "0.11"
```

```rust
fn main() -> Result<()> {
    env_logger::init();
    // ... rest unchanged
}
```

```bash
RUST_LOG=debug cargo run --example extract -- invoice.pdf
```

### Per-page stats

```rust
for page in doc.pages() {
    let page = page?;
    let chars = page.chars()?;
    let words = page.words(&WordOptions::default())?;
    eprintln!("page {}: {} chars, {} words", page.index(), chars.len(), words.len());
}
```

## Related reading

- [[guides/quick-start]]
- [[examples/invoice-snapshot]]
- `examples/extract.rs` in the repo
