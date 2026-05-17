---
tags: [guide, tuning, options]
---

# Tuning `TextOptions`

When to deviate from pdfplumber defaults and in which direction.

## Defaults

```rust
TextOptions {
    x_tolerance:       3.0,      // pt
    y_tolerance:       3.0,      // pt
    x_tolerance_ratio: None,
    x_density:         7.25,     // pt per virtual column
    y_density:         13.0,     // pt per virtual line
    keep_blank_chars:  false,
    use_text_flow:     false,
    expand_ligatures:  false,
}
```

Think of the parameters as two pairs:

- `*_tolerance` controls **grouping** (what counts as "the same word"
  or "the same line").
- `*_density` controls the **rendering** of the output (how dense the
  string is).

## Fluent (builder) pattern

```rust
let opts = TextOptions::builder()
    .x_tolerance(1.5)
    .y_density(11.0)
    .expand_ligatures(true)
    .build();
```

## Decision by symptom

### "Words break where they shouldn't"

You see `"Hello"` split as `"Hell"` `"o"`.

→ **Raise `x_tolerance`** (e.g. 4–6). Or try
`x_tolerance_ratio: Some(0.5)` for dynamic tolerance.

### "Two words glued as one"

You see `"$100Total"` when they should be separate.

→ **Lower `x_tolerance`** to 1–2. Careful: breaks more things.

### "Lines blend together"

Text from two distinct lines appears as one.

→ **Lower `y_tolerance`** to 2.

### "Subscripts/superscripts fall onto another line"

Small chars above or below a word get separated.

→ **Raise `y_tolerance`** to 5–6.

### "Columns are misaligned"

`extract_text_layout` produces columns that don't match the PDF
visually.

→ Look at the PDF's fonts. If the nominal font is very wide (e.g.
Courier) your `x_density=7.25` (geared for Helvetica) underestimates.
Try **raising `x_density`** to 9–10.

### "Too much vertical whitespace between paragraphs"

The output has 4–5 newlines between lines that are 2 cm apart in the
PDF.

→ **Raise `y_density`** to 16–18.

### "Output too cramped, no breathing room between sections"

Opposite: PDF-separated sections appear glued in the output.

→ **Lower `y_density`** to 10–11.

### "Weird characters like `\u{0001}` show up"

The embedded font has no `/ToUnicode` and the codes are not mapping.

→ No tuning fixes this. It is a bug from the PDF generator. Workaround:
detect via `c.text.chars().all(|ch| ch.is_control())` and report. See
[[reference/font-encodings]].

### "Unicode ligatures (ﬁ, ﬃ) appear in the output"

If you're going to regex over the output, the FB00–FB06 chars will
trip you up.

→ **`.expand_ligatures(true)`**: turns `ﬁ` into `fi` before building
the words.

### "The PDF's literal spaces matter"

For example, a monospaced font that already inserts spaces between
words with 0x20 bytes.

→ **`.keep_blank_chars(true)`** preserves those spaces as explicit
chars instead of treating them as separators.

### "I want stream order, not by coordinates"

Some (rare) PDFs depend on text being read in stream order.

→ **`.use_text_flow(true)`**. Disables sort by `(top, x0)`.

## Pocket table

| Problem | Setting | Direction |
|----------|---------|-----------|
| Words broken | `x_tolerance` | ⬆ |
| Words glued | `x_tolerance` | ⬇ |
| Lines merged | `y_tolerance` | ⬇ |
| Subscripts in wrong line | `y_tolerance` | ⬆ |
| Misaligned columns | `x_density` | ⬆ (Courier-like) or ⬇ (condensed) |
| Too much vertical whitespace | `y_density` | ⬆ |
| Output too cramped | `y_density` | ⬇ |
| Mixed fonts (very different sizes) | `x_tolerance_ratio` | `Some(0.5)` |
| Unicode ligatures annoying | `expand_ligatures` | `true` |
| PDF spaces matter | `keep_blank_chars` | `true` |
| Stream order | `use_text_flow` | `true` |

## Recommended workflow

1. Start with `TextOptions::pdfplumber_defaults()`.
2. Look at the output.
3. Apply **one change at a time** and compare.
4. When you hit the sweet spot, save your config as `fn my_opts() -> TextOptions`.

## Related reading

- [[architecture/layout-algorithm]] — what each parameter does inside
- [[guides/extracting-invoices]] — real tuning cases
