---
tags: [adr, accepted, performance]
---

# ADR 0006 — `Cow<[Char]>` in `extract_words`

**Status**: Accepted · **Date**: 2026-05-17

## Context

`extract_words` can operate in two modes:

- **Without ligature expansion** (default case): the input slice is OK
  as-is.
- **With expansion** (`opts.expand_ligatures == true`): every `ﬁ` is
  replaced with `fi`, etc., which requires building a new slice.

The first implementation always cloned:

```rust
let working: Vec<Char> = if opts.expand_ligatures {
    chars.iter().map(expand_char).collect()
} else {
    chars.to_vec()    // ⚠️ unnecessary clone
};
```

`Char` carries two `CompactString` (24 inline bytes each), a bunch of
`f32`s, and a `bool`. Cloning N chars in the default case was the
largest avoidable expense in the pipeline.

## Decision

We use `std::borrow::Cow<'_, [Char]>`:

```rust
let working: Cow<'_, [Char]> = if opts.expand_ligatures {
    Cow::Owned(chars.iter().map(expand_char).collect())
} else {
    Cow::Borrowed(chars)
};
```

`&working` auto-derefs to `&[Char]`, so the rest of the code
(`cluster_objects`, sort, walk) needs no changes.

## Consequences

### ✅ Advantages

- **Zero allocations** in the default case (`expand_ligatures=false`).
- **Same cost** as before when expansion is requested.
- **No semantic trade-off**: the output `Vec<Word>` is identical.

### ❌ Costs

- One more abstraction (`Cow`) that the reader has to know. It is
  standard Rust ergonomics so it's acceptable.

## Verification

`cargo bench` (when we add it) should show measurable reduction in
`extract_words` for PDFs with ≥1k chars. The snapshot test
`tests/snapshots/extraction__layout_snapshot_invoice.snap` confirms
that the functional output didn't change.

## Generalizable lesson

`Cow<[T]>` is the right tool when:

1. A function takes a slice.
2. Under certain options, it needs to transform it before using.
3. Under other options, the original slice works as-is.

Always borrowing forces a clone that is waste. Computing conditionally
with `if/else` forces you to Box the iterator. `Cow` is the idiomatic
solution.

## Related reading

- [Apollo Rust Best Practices — Chapter 1.1](https://github.com/apollographql/rust-best-practices/blob/main/chapter_01.md)
  "Borrowing over Cloning"
- `src/text/extractor.rs::extract_words`
