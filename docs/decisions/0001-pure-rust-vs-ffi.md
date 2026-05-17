---
tags: [adr, accepted]
---

# ADR 0001 — Pure Rust vs FFI

**Status**: Accepted · **Date**: 2026-05-17

## Context

To extract text from PDFs there are two paths:

1. **Pure Rust**: write the parser or build on `lopdf` / `pdf-rs`.
2. **FFI**: bindings to `PDFium` (Chromium), `MuPDF`, or `Poppler`.

The FFI options are battle-tested against real-world dirty PDFs; the
pure-Rust ones are more portable, dylib-free, easy to cross-compile to
Linux/Mac/Win/WASM, and allow any commercial use without license
restrictions.

## Decision

**We go pure Rust.** The backend is
[[decisions/0002-lopdf-as-backend|lopdf]].

## Consequences

### ✅ What we gain

- **Zero dylibs**. `cargo build` produces a self-contained binary.
- **Trivial cross-compile** to any Rust target (including WASM).
- **Clean licensing**: permissive (MIT) without AGPL contamination
  (mupdf-rs) or GPL chain (poppler-rs).
- **No C/C++ build toolchain** required in CI.
- **Direct debugging** with `cargo` and rust-analyzer, no jumping to
  external symbols.

### ❌ What we lose

- **Less edge-case coverage**. PDFium handles 20 years of weird real-
  world PDFs; lopdf doesn't. We will trip over malformed fonts,
  incomplete ToUnicode, etc.
- **No rendering or rasterization** (irrelevant for our scope, but
  limits future extensions).
- **Raw performance may be lower** than PDFium on huge PDFs.

## Alternatives considered

### `pdfium-render` (FFI to Chromium's PDFium)

- ➕ Battle-tested, gives `PdfPageTextChar` with bbox/font/size per
  character.
- ➖ Requires shipping `libpdfium.so/dylib` (~10 MB). Complex
  cross-compile. Apache+BSD but drags a C++ toolchain.

### `mupdf-rs` (FFI to MuPDF)

- ➕ Excellent coverage, native StructuredText with hierarchy.
- ➖ **AGPL-3.0**. Disqualifies any closed-source product except via
  a commercial Artifex license. Blocker.

### `poppler-rs` (GLib wrapper)

- ➕ Mature on Linux.
- ➖ GPL/LGPL chain, system-lib dependency, effectively Linux-only.

### Hand-rolled parser

- ➕ Total control.
- ➖ Re-implementing xref, filters (FlateDecode), objects, content
  streams is many weeks. Doesn't add differentiating value.

## Re-evaluation

If we later find that real invoice PDFs break systematically and
improving `lopdf` is not feasible, we would open a new ADR proposing
`pdfium-render` as an optional feature flag.

## Related reading

- [[decisions/0002-lopdf-as-backend]] — why lopdf among the pure-Rust
  options
- [[architecture/overview]]
