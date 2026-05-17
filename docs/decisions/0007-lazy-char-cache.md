---
tags: [adr, accepted, caching]
---

# ADR 0007 — Lazy per-page char cache

**Status**: Accepted · **Date**: 2026-05-17

## Context

`Page::chars()` walks the content stream and emits a `Vec<Char>`. For
a user that calls `extract_text_layout` + `words` + `chars` on the same
page, doing that work three times would be waste.

But also: if the user never calls `chars()` (because they only care
about `is_scanned()`), we don't want to have parsed the content stream
for nothing.

We need a **lazy** and **thread-safe** cache (even though the crate is
not `Sync`-friendly yet, the API must allow multi-thread when
[[roadmap|rayon-parallel pages]] lands).

## Decision

`Page<'doc>` has two caches:

```rust
pub struct Page<'doc> {
    doc: &'doc Document,
    index: usize,
    chars_cache:   OnceLock<Vec<Char>>,
    metrics_cache: OnceLock<PageMetrics>,
}
```

`OnceLock<T>` is the stable std API for "single, thread-safe, lock-free
init after the first write".

On error, **we don't cache** — the next call retries:

```rust
pub fn chars(&self) -> Result<&[Char]> {
    if let Some(v) = self.chars_cache.get() {
        return Ok(v.as_slice());
    }
    let v = crate::parser::lopdf_backend::extract_chars(self)?;
    match self.chars_cache.set(v) {
        Ok(()) | Err(_) => {}
    }
    Ok(self.chars_cache.get().map(Vec::as_slice).unwrap_or_default())
}
```

## Consequences

### ✅ Advantages

- **Truly lazy**: if you don't call
  `chars()`/`words()`/`extract_text*()`, we never parse the content
  stream.
- **Idempotent**: calling `chars()` N times parses once.
- **No `Mutex`**: `OnceLock` uses atomics, much cheaper.
- **Recoverable errors**: if the first call fails, the second retries.
  Useful when something in the environment changes between calls
  (unlikely, but a good property).

### ❌ Costs

- If `chars()` is called from multiple threads simultaneously for the
  same page, two may start parsing, but only one wins the `set()`; the
  other drops its `Vec<Char>`. That is occasional waste, not a bug.

## Why not a `Document`-level cache?

We considered caching `Vec<Vec<Char>>` (one slot per page) in
`Document`. Decision: **no**, because:

1. `Document` is already dense enough (owns the `lopdf::Document`).
2. If you want to process 1000 pages in parallel with rayon, the per-
   page cache lets each worker own its `Page<'doc>` without contention.
3. If you never touch a page, you don't pay its memory.

## Alternatives considered

### `Mutex<Option<Vec<Char>>>`

- ➖ Lock contention on reads. `OnceLock` is lock-free after the first
  write.

### Eager parsing in `Document::open`

- ➕ Errors are detected up front.
- ➖ Expensive for huge PDFs where only one page is used.

### No cache (re-parse every time)

- ➖ Trivially slower. `extract_text_layout` internally calls
  `chars()`; without cache, every public-method call would re-parse.

## Related reading

- [`std::sync::OnceLock` docs](https://doc.rust-lang.org/std/sync/struct.OnceLock.html)
- `src/page.rs::chars`
