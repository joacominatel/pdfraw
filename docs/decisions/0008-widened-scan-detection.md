---
tags: [adr, accepted]
---

# ADR 0008 — Widen `is_scanned()` heuristic

**Status**: Accepted · **Date**: 2026-05-17

## Context

Benchmarking against ~40 real invoice PDFs surfaced a false negative:
`Comprobante.pdf` produces zero chars yet `is_scanned()` returned
`false`. Inspecting its content stream showed a single `Do /Im1`
invocation with no `BT`/`Tj`/`TJ` operators — clearly image-only — but
the previous heuristic required the referenced XObject to resolve to
`/Subtype /Image` via `get_page_resources`. For this PDF the lookup
returned `None` (resources live somewhere the simple lookup did not
reach), so the verification failed silently and we reported "not
scanned".

The semantic question is: **what does `is_scanned() == true` mean to
the caller?** The honest answer is "this page will give you no
extractable text via `extract_text*`; route to OCR if you need its
contents".

## Decision

We replace the previous criterion with a simpler one that matches that
semantic:

> A page is scanned when its content stream emits **no text-show
> operators** and emits **at least one image-bearing operator** — any
> `Do` (no subtype check) or any `BI` (inline image).

Concretely:

```rust
let mut has_text = false;
let mut has_image_like = false;
for op in &content.operations {
    match op.operator.as_str() {
        "BT" | "Tj" | "TJ" | "'" | "\"" => has_text = true,
        "Do" | "BI"                     => has_image_like = true,
        _ => {}
    }
    if has_text { return Ok(false); }
}
Ok(has_image_like)
```

No more resource-dictionary walking, no more `Subtype` check, no more
silent `Ok(false)` when a lookup fails.

## Consequences

### ✅ What we gain

- **Correctly flags `Comprobante.pdf`-style PDFs** where the scan is
  wrapped in a Form XObject or where resource resolution is awkward.
- **Cheaper**: one linear pass over the operator vector, no dictionary
  resolution, no per-operator allocations.
- **Robust to the Pages-tree inheritance** of resources — the heuristic
  works regardless of where `/XObject` lives.
- **Better aligned with the public contract**: a page that draws only
  a Form XObject is also a page from which we cannot extract text
  today, because our state machine does not descend into Form XObjects
  either.

### ❌ What we accept

- A page that draws **only** a Form XObject **whose content is real
  text** would be reported as `is_scanned == true` even though that
  text *could* in principle be extracted. We do not extract it today,
  so the report is consistent with observable behaviour. If we ever
  start descending into Form XObjects, this ADR will need a follow-up.
- Marginal: a page with **text only inside a Form XObject** plus
  decorative paths and images would also be reported as scanned. Same
  reasoning.

## Verification

- `tests/extraction.rs::text_page_is_not_reported_as_scanned` still
  passes (negative case).
- New unit tests in `src/parser/scan_detect.rs`:
  - `returns_false_when_page_has_text`
  - `returns_true_when_page_has_only_do_invocation` — synthesises a
    page that mirrors the `Comprobante.pdf` shape.
  - `returns_false_when_page_is_empty`
  - `returns_false_when_page_has_only_path_operators`
- Real-world: benchmark against the 40-PDF corpus now reports `1 with
  scanned pages` instead of `0`, matching manual inspection.

## Alternatives considered

### Keep the subtype check, fall back to "has text == false" when it fails

Adds complexity without changing the outcome — if the lookup fails we
end up at the same answer the simpler heuristic gives.

### Add a richer `PageTextStatus` enum (`Text` / `Scanned` /
`VectorOnly` / `Empty`)

Better semantics, but breaking API for v0.1. Tracked in
[[roadmap]] as a v0.2 idea.

## Related reading

- [[architecture/state-machine]] — where the heuristic lives
- `src/parser/scan_detect.rs`
- [[decisions/0004-mvp-scope]] — why we don't OCR ourselves
