---
tags: [adr, accepted, error-handling]
---

# ADR 0005 — `Error` enum design

**Status**: Accepted · **Date**: 2026-05-17

## Context

There are several valid ways to model errors in a Rust library:

- A single struct `pub struct Error { kind: ErrorKind, ... }`.
- An enum without `non_exhaustive` (breaks semver when adding
  variants).
- An enum `#[non_exhaustive]` with `thiserror`.
- `Box<dyn std::error::Error>` (easy to propagate, bad to match on).
- `anyhow::Error` (only recommended for binaries, not libs).

The Apollo handbook (chapter 4) is clear: **`thiserror` for libs,
`anyhow` for binaries**.

## Decision

`Error` enum with `thiserror::Error` and `#[non_exhaustive]`. Alias
`pub type Result<T> = std::result::Result<T, Error>`.

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("PDF parse error: {0}")]
    Pdf(#[from] lopdf::Error),

    #[error("invalid content stream on page {page}: {reason}")]
    ContentStream { page: usize, reason: String },

    #[error("unsupported feature: {0}")]
    Unsupported(&'static str),

    #[error("font decoding failed: {0}")]
    Font(String),

    #[error("page index {0} out of bounds")]
    PageOutOfBounds(usize),
}
```

## Consequences

### ✅ Advantages

- **`#[from]`** on `Io` and `Pdf` lets us use `?` directly when the
  underlying error is from I/O or lopdf.
- **`#[non_exhaustive]`** lets us add future variants
  (`Encrypted`, `Ocr`, etc.) without breaking semver — callers always
  need a `_` arm.
- **Structured variants** (`ContentStream { page, reason }`) give
  callers data for routing/logging without parsing strings.
- **Inline messages** with `#[error("...")]` keep them next to the
  variant.

### ❌ Accepted costs

- `String` in `ContentStream.reason` and `Font(String)` costs an
  allocation per error. Acceptable because errors are rare events.
- Callers `match`ing on the enum need an extra `_` arm because of
  `non_exhaustive`.

## Rules we follow

1. **Never `unwrap`/`expect` in non-test code**. If you need it,
   convert it to a `Result` with a new variant.
2. **Every fallible function has a `# Errors` rustdoc section**.
3. **Structured errors carry useful info**, not just strings.
4. **Propagate with `?`**, not with `match` chains. If you need to
   change the error type, `.map_err(...)?`.

## Alternatives considered

### Single `Other(Cow<'static, str>)` variant

- ➖ Loses kind-based routing. Callers can't tell "invalid PDF" from
  "I/O failure" without parsing strings.

### `anyhow::Error`

- ➖ Forbidden by the handbook in libraries. Acceptable only for
  binaries.

### Struct error with `kind: enum ErrorKind`

- ➖ More boilerplate without real advantage. The "kind enum" is what
  we're doing, just without the struct indirection.

## Related reading

- [Apollo Rust Best Practices — Chapter 4](https://github.com/apollographql/rust-best-practices/blob/main/chapter_04.md)
- `src/error.rs`
