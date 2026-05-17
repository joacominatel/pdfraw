---
tags: [architecture, algorithm, layout]
---

# Layout algorithm (stages 2 + 3)

> File: `src/text/extractor.rs`
> Faithful port of `pdfplumber.extract_text(layout=True)`. The defaults
> are identical to the Python library: `x_density=7.25`,
> `y_density=13.0`, `x_tolerance=3.0`, `y_tolerance=3.0`.

Stage 1 gave you a `Vec<Char>` with absolute positions in pt. Stage 2
groups them into `Word`s, and stage 3 emits a `String` where the columns
are preserved.

## Stage 2: `extract_words`

Three passes over the chars:

### Pass 1 — sort

```rust
chars sorted by (top ascending, x0 ascending)
```

Unless `opts.use_text_flow == true`, in which case we keep the PDF
stream order.

### Pass 2 — cluster into lines

```rust
cluster_objects(chars, |c| c.top, y_tolerance) -> Vec<Vec<&Char>>
```

`cluster_objects` (in `text/cluster.rs`) is a greedy 1D pass: sort by
key, then sweep grouping while `keyN <= keyN-1 + tolerance`. O(n log n)
because of the sort.

### Pass 3 — split into words

Within each line, sort by `x0`, then walk with this rule:

```rust
fn char_begins_new_word(prev, curr, opts) -> bool {
    // 1. Different line (defensive sanity check)
    if (prev.top - curr.top).abs() > opts.y_tolerance { return true; }

    // 2. Strong backtrack
    if curr.x0 + opts.x_tolerance < prev.x0 { return true; }

    // 3. Big horizontal gap
    let x_tol = opts.x_tolerance_ratio
        .map(|r| r * prev.size)             // dynamic per size
        .unwrap_or(opts.x_tolerance);       // fixed in pt
    curr.x0 > prev.x1 + x_tol
}
```

`keep_blank_chars=false` (default) also makes a space character act as
a word separator.

### Output

```rust
struct Word {
    text:       CompactString,
    x0, x1:     f32,           // min/max across chars
    top, bottom: f32,
    char_range: Range<usize>,   // indices into chars()
}
```

## Stage 3: `extract_text_layout` — the virtual grid

This is the part that sets the crate apart from a "join words with
spaces".

### Conceptually

Imagine we split the page into a **virtual monospaced grid** of:

- `x_density = 7.25 pt` wide per column.
- `y_density = 13.0 pt` tall per row.

The grid origin is the top-left corner of the bbox that wraps **all
chars** on the page (not the page itself).

Each word falls in a specific **virtual column**:

```
col_of(word) = round((word.x0 - x_origin) / x_density)
row_of(line) = round((line.top  - y_origin) / y_density)
```

The output is a string where each word lives in the virtual column it
belongs to, padded with `' '` for alignment and `'\n'` for line breaks.

### Exact pseudocode

```rust
let words = extract_words(chars, opts);          // already sorted (top, x0)
let lines = group(words, top, y_tolerance);      // words → lines
let x_origin = min(c.x0 for c in chars);
let y_origin = min(c.top for c in chars);

let mut out = String::new();
let mut newlines_so_far = 0;

for (i, line) in lines.enumerate() {
    let target_row = round((line.top - y_origin) / y_density);
    let minimum = if i > 0 { 1 } else { 0 };
    let needed_newlines = max(target_row - newlines_so_far, minimum);
    out.extend(repeat_n('\n', needed_newlines));
    newlines_so_far += needed_newlines;

    let mut col = 0;
    for (j, word) in line.enumerate() {
        let target_col = round((word.x0 - x_origin) / x_density);
        let minimum = if j > 0 { 1 } else { 0 };
        let needed_spaces = max(target_col - col, minimum);
        out.extend(repeat_n(' ', needed_spaces));
        col += needed_spaces;

        out.push_str(&word.text);
        col += word.text.chars().count();
    }
}
```

`max(..., minimum)` guarantees there is always at least one separator
between consecutive items, even when the virtual-column math would
disagree.

## Why this preserves invoices

A typical invoice has three logical "columns":

```
ACME Corp                                Invoice #001
                                         Date: 2026-05-17

Item A                                                  $10.00
Item B                                                  $20.00
                                            ─────────────────
Total                                                   $30.00
```

Each of those numbers is rendered by a `Tj` at an x position much
larger than the descriptive text on the left. The algorithm:

1. Clustering by `top` puts them on the **same line** (if they share y).
2. Clustering by `x0` after sort doesn't join them into a single word
   (because the gap exceeds `x_tolerance`).
3. The virtual grid assigns `$10.00` to column `round(500/7.25) ≈ 69`
   and `Item A` to column 0. Result: ~67 spaces between them.

The final number lands in **the same column** as the subtotal above it,
just like in the original PDF.

See [[examples/invoice-snapshot]] for a real case.

## Tuning

Each parameter affects a specific area. [[guides/tuning-text-options]]
has a "when to raise / when to lower" cheatsheet.

| Parameter | Raise when… | Lower when… |
|-----------|-------------|--------------|
| `x_tolerance` | Words stick together with wide kerning | Dense columns blend |
| `y_tolerance` | Subscripts get pushed to another line | Tightly packed lines merge |
| `x_density` | Lots of spaces between columns | Output looks cramped |
| `y_density` | Too much vertical whitespace between paragraphs | Output looks stretched |
| `x_tolerance_ratio` | Mixed fonts (same PDF, very different sizes) | — |

## Algorithm edge cases

- **Very dense document** — if two words are closer than `x_tolerance`
  but clearly not the same word (e.g. `Total` and `$10` with a tab-like
  hyphen), you'll see them joined. Fix: lower `x_tolerance` or raise
  `x_tolerance_ratio`.
- **Subscripts** — characters with slightly different `top` can break
  lines. Bump `y_tolerance` to 5–6 pt.
- **Rotated text** — `extract_text_layout` includes it because
  `extract_words` does not filter on `upright=false`. If that bothers
  you, filter beforehand:
  `let upright: Vec<_> = page.chars()?.iter().filter(|c| c.upright).cloned().collect();`

## Related reading

- [[architecture/overview]] — where this stage fits
- [[guides/tuning-text-options]] — how to tune for your particular PDF
- [[decisions/0003-pdfplumber-algorithm]] — why we exactly replicate
  pdfplumber's defaults
