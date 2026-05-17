---
tags: [adr, accepted]
---

# ADR 0003 — Replicate the pdfplumber algorithm

**Status**: Accepted · **Date**: 2026-05-17

## Context

To extract text while preserving layout, we had several possible
strategies:

1. **Plain text** (à la `pdftotext`) — the PDF's chars in stream order,
   no positioning.
2. **Custom layout heuristic** — invent a layout reconstruction
   algorithm.
3. **Replicate pdfplumber** — the Python community's reference tool
   for invoices / data engineering.

The user explicitly named pdfplumber as the desired behavior: "the goal
is for this to take a PDF and return the raw text, but with spaces,
line breaks, all the information exact".

## Decision

We replicate **`pdfplumber.Page.extract_text(layout=True)`** bit by
bit:

- Same defaults: `x_tolerance=3.0`, `y_tolerance=3.0`,
  `x_density=7.25`, `y_density=13.0`.
- Same three passes: sort → cluster into lines → split into words.
- Same formula for `num_newlines` and `num_spaces` in the virtual grid.
- Same behavior for `keep_blank_chars`, `use_text_flow`,
  `x_tolerance_ratio`.

## Consequences

### ✅ Advantages

- **Cross-validation for free**: we can run pdfplumber on the same PDFs
  and diff the outputs. Serious discrepancies indicate a bug in our
  implementation.
- **Implicit documentation**: there are books, blog posts and
  StackOverflow with pdfplumber code that maps 1:1 to our API.
- **Users coming from Python have a smooth runway**.

### ❌ Disadvantages

- **We inherit its limitations**: rotated text is treated as upright,
  subscripts can break lines with too-low `y_tolerance`, etc.
- **We can't easily innovate** without diverging from the reference.
  Trade-off accepted.

## Algorithm detail

Documented in [[architecture/layout-algorithm]].

## Verification

We keep an `insta` snapshot of the output over a synthetic invoice (see
`tests/snapshots/extraction__layout_snapshot_invoice.snap`). The format
is what pdfplumber would produce with the same defaults.

Future validation planned (see [[roadmap]]):

```bash
# tests/cross_validate.sh
python -c "import pdfplumber; print(pdfplumber.open('sample.pdf').pages[0].extract_text(layout=True))" > expected.txt
cargo run --example extract -- sample.pdf > got.txt
diff expected.txt got.txt
```

## Alternatives considered

### Plain text à la `pdftotext`

- ➕ Simple, already implemented as `extract_text_simple`.
- ➖ Useless for invoices: loses the item ↔ price association.

### Custom layout heuristic

- ➕ We could optimize for invoices specifically.
- ➖ Reinventing the wheel. pdfplumber is already battle-tested in
  thousands of companies.

## Related reading

- [[architecture/layout-algorithm]] — implementation of the decision
- [[guides/tuning-text-options]] — how to deviate from defaults
