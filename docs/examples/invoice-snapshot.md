---
tags: [example, invoice, snapshot]
---

# Example: synthetic invoice with snapshot

This page documents the `layout_snapshot_invoice` test in
`tests/extraction.rs:140`. It is the case we validate automatically as
"the layout algorithm is still correct".

## Input — synthetic PDF

`printpdf` generates a PDF with these texts on A4 portrait
(210 × 297 mm), Helvetica 12pt:

```
position (mm)        text
─────────────────    ──────────
(20, 270)            ACME
(150, 270)           Invoice
(20, 240)            Line A
(170, 240)           1.00
(20, 225)            Line B
(170, 225)           2.00
(20, 200)            Total
(170, 200)           3.00
```

Notice **ACME** and **Invoice** share `y=270` (same line), as does
each item with its amount.

## Test code

```rust
#[test]
fn layout_snapshot_invoice() {
    let pdf = common::build_pdf(&[
        (20.0, 270.0, "ACME"),
        (150.0, 270.0, "Invoice"),
        (20.0, 240.0, "Line A"),
        (170.0, 240.0, "1.00"),
        (20.0, 225.0, "Line B"),
        (170.0, 225.0, "2.00"),
        (20.0, 200.0, "Total"),
        (170.0, 200.0, "3.00"),
    ]);
    let doc = Document::from_bytes(pdf).unwrap();
    let layout = doc
        .page(0)
        .unwrap()
        .extract_text_layout(&TextOptions::pdfplumber_defaults())
        .unwrap();
    insta::assert_snapshot!(layout);
}
```

## Output (accepted snapshot)

`tests/snapshots/extraction__layout_snapshot_invoice.snap`:

```
ACME                                               Invoice






Line A                                                     1.00


Line B                                                     2.00




Total                                                      3.00
```

## Why this output

We convert mm → pt: 1 mm ≈ 2.835 pt. And `printpdf` places texts using
`y` from the bottom, so:

- `y_mm=270` → `top_pt ≈ 297 - 270 = 27 mm = 76.5 pt` from the top.
- `x_mm=150` → `x_pt = 150 * 2.835 = 425 pt`.

With `x_density=7.25`:
- `ACME` starts at x ≈ 56 pt → column `round(56/7.25) ≈ 8`.
- `Invoice` starts at x ≈ 425 pt → column `round(425/7.25) ≈ 59`.
- Between them: 59 - 8 - len("ACME") = 47 spaces… (approximated by
  the algorithm's bbox math).

The output shows ~51 spaces between "ACME" and "Invoice", reflecting
the virtual column of `Invoice` after laying down "ACME" plus a small
delta.

## How to regenerate the snapshot

If you deliberately change the algorithm:

```bash
# 1. Delete the old snapshot
rm tests/snapshots/extraction__layout_snapshot_invoice.snap

# 2. Run the test (a .snap.new will be produced)
INSTA_UPDATE=auto cargo test --test extraction layout_snapshot_invoice

# 3. Review it
mv tests/snapshots/extraction__layout_snapshot_invoice.snap.new \
   tests/snapshots/extraction__layout_snapshot_invoice.snap

# 4. Commit
git add tests/snapshots/extraction__layout_snapshot_invoice.snap
```

Or, with `cargo-insta` installed:

```bash
cargo install cargo-insta
cargo insta review
```

More detail in [[testing/snapshot-workflow]].

## Other integration tests

- `layout_preserves_invoice_columns` (`tests/extraction.rs:100`) —
  property-style case with an even more realistic invoice (Acme Corp +
  4 items + Total).
- `multi_page_document_iterates_in_order` — two pages, validates that
  order and extraction are stable.

See [[testing/strategy]] for the full pyramid.
