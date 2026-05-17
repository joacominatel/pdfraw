---
tags: [guide, invoices]
---

# Extracting invoices: real patterns

A typical invoice has these spatial elements:

```
┌─────────────────────────┬───────────────────────────────┐
│ LOGO                    │  Right header (issuer)        │
├─────────────────────────┴───────────────────────────────┤
│  Recipient data                                          │
│                                                          │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  Item                            Qty   Price  Total │ │
│  │  ─────────────────────────────────────────────────  │ │
│  │  Service A                       1    100.00  100.00│ │
│  │  Product B                       2     50.00  100.00│ │
│  └─────────────────────────────────────────────────────┘ │
│                                                          │
│                                          Subtotal: 200.00│
│                                          VAT 21%:   42.00│
│                                          Total:    242.00│
└─────────────────────────────────────────────────────────┘
```

The crate **preserves this structure** in `extract_text_layout`. Your
logic only needs to scrape the text output, not compute positions.

## Pattern 1 — parse the layout text with regex

```rust
use pdfraw::prelude::*;
use regex::Regex;

fn main() -> Result<()> {
    let doc = Document::open("invoice.pdf")?;
    let text = doc.page(0)?.extract_text_layout(&TextOptions::pdfplumber_defaults())?;

    // Items: description followed by Qty, unit price, total (all right-aligned)
    let item_re = Regex::new(r"^(?P<desc>\S.+?)\s{2,}(?P<qty>\d+)\s+(?P<unit>[\d.,]+)\s+(?P<total>[\d.,]+)\s*$").unwrap();
    for line in text.lines() {
        if let Some(cap) = item_re.captures(line) {
            println!("{} × {} = {}", &cap["desc"], &cap["qty"], &cap["total"]);
        }
    }

    // Final total: "Total:" followed by a right-aligned number
    let total_re = Regex::new(r"Total:\s+([\d.,]+)").unwrap();
    if let Some(cap) = total_re.captures(&text) {
        println!("TOTAL: {}", &cap[1]);
    }
    Ok(())
}
```

Works because the spaces between `Total:` and the amount are part of
the output, pdfplumber-style.

## Pattern 2 — extract by coordinates (more robust)

When there are font variations or each vendor designs the invoice
differently, **working with coordinates** from `Page::words()` is more
reliable:

```rust
use pdfraw::prelude::*;

fn main() -> Result<()> {
    let doc = Document::open("invoice.pdf")?;
    let page = doc.page(0)?;
    let words = page.words(&WordOptions::default())?;

    // Assume the "Total" column is around x ≈ 500.
    // Look for words with x0 > 480 and x1 < 560 (defensive window).
    let total_column: Vec<_> = words.iter()
        .filter(|w| w.x0 > 480.0 && w.x1 < 560.0)
        .collect();

    for w in total_column {
        println!("@ y={:.0}: {}", w.top, w.text);
    }
    Ok(())
}
```

This approach is immune to PDF spacing changes; it only depends on the
total column staying in the same x strip.

## Pattern 3 — detect scanned pages and route to OCR

```rust
use pdfraw::prelude::*;

#[derive(Debug)]
enum PageContent {
    Text(String),
    NeedsOcr,
}

fn process(doc: &Document) -> Result<Vec<PageContent>> {
    let mut out = Vec::new();
    for page in doc.pages() {
        let page = page?;
        if page.is_scanned()? {
            out.push(PageContent::NeedsOcr);
        } else {
            let text = page.extract_text_layout(&TextOptions::pdfplumber_defaults())?;
            out.push(PageContent::Text(text));
        }
    }
    Ok(out)
}
```

For the OCR you can hand the page to Tesseract, AWS Textract or Google
Document AI depending on your stack. See [[roadmap]] for future native
integration.

## Pattern 4 — multi-page with accumulated totals

Some invoices span multiple pages: items on page 1 and 2, totals on
page 2. Use `doctop` (top accumulated across pages) on `Char` to
associate items:

```rust
use pdfraw::prelude::*;

fn all_words_globally_sorted(doc: &Document) -> Result<Vec<(usize, pdfraw::Word)>> {
    let mut out = Vec::new();
    for page in doc.pages() {
        let page = page?;
        for w in page.words(&WordOptions::default())? {
            out.push((page.index(), w));
        }
    }
    // `doctop` is not exposed on Word yet; use the page index + top
    // tuple as the key:
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.top.partial_cmp(&b.1.top).unwrap()));
    Ok(out)
}
```

> Note: in the MVP `Word` does not expose `doctop`. It is in [[roadmap]]
> to add it for multi-page cases.

## When `extract_text_layout` can betray you

Things to watch:

- **Subtitles overlapping the line above** — drop `y_tolerance` to
  2 pt.
- **Justified text with wide word spacing** — bump `x_tolerance` to
  6–8 pt or use `x_tolerance_ratio = 0.5`.
- **Custom embedded fonts** — check `c.text` on the chars; if you see
  `\u{0001}` the font has no `/ToUnicode` and the caller needs to flag
  the issue. See [[reference/font-encodings]].

## Related reading

- [[guides/tuning-text-options]]
- [[examples/invoice-snapshot]]
- [[architecture/layout-algorithm]]
