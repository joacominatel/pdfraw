---
tags: [index, map-of-content]
---

# Vault map

> Bird's-eye view of the whole documentation set. For a narrative
> landing page start at [[README]].

## 🚀 Quick links

- [[guides/quick-start]] — open a PDF and extract layout-preserving text
- [[reference/api]] — public surface (types and methods)
- [[architecture/overview]] — the 4 pipeline stages
- [[decisions/index]] — why we decided what we decided
- [[roadmap]] — what's missing before v0.2

## 🏗️ Architecture

- [[architecture/overview]] — Document → Page → Char → Word → Text
- [[architecture/module-map]] — a guided tour of each module
- [[architecture/state-machine]] — how we walk the PDF content stream
- [[architecture/layout-algorithm]] — the monospaced virtual grid

## 📜 Decisions (ADRs)

- [[decisions/0001-pure-rust-vs-ffi]] — why we don't use `pdfium`/`mupdf`
- [[decisions/0002-lopdf-as-backend]] — what `lopdf` gives us and what it doesn't
- [[decisions/0003-pdfplumber-algorithm]] — why we replicate pdfplumber
- [[decisions/0004-mvp-scope]] — no OCR, no tables, no encryption in v0.1
- [[decisions/0005-error-type-design]] — `thiserror`, `#[non_exhaustive]`
- [[decisions/0006-cow-in-word-extraction]] — borrow over clone on the hot path
- [[decisions/0007-lazy-char-cache]] — per-page `OnceLock<Vec<Char>>`

## 📘 Guides

- [[guides/quick-start]]
- [[guides/extracting-invoices]] — real patterns for the target use case
- [[guides/tuning-text-options]] — `x_tolerance`, `y_density`, ratios

## 🔧 Reference

- [[reference/api]] — every public type with examples
- [[reference/pdf-operators-supported]] — what the state machine understands
- [[reference/font-encodings]] — WinAnsi, ToUnicode, Differences, Type0

## 💡 Examples

- [[examples/cli-extract]] — the `examples/extract.rs` binary
- [[examples/invoice-snapshot]] — input PDF → layout-preserving output

## 🧪 Testing

- [[testing/strategy]] — pyramid: unit / integration / doctest / snapshot
- [[testing/snapshot-workflow]] — how to accept/review `insta` snapshots

## 📚 Support

- [[glossary]] — `BT`, `Tj`, `CMap`, `MediaBox`, etc.
- [[roadmap]] — prioritized backlog
