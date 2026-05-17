---
tags: [decisions, adr-index]
---

# Architecture Decision Records (ADRs)

Each ADR is an **immutable** record of an important decision. If a
decision changes, a new ADR is opened that supersedes the old one, and
they link to each other.

| # | Title | Status |
|----|--------|--------|
| 0001 | [[decisions/0001-pure-rust-vs-ffi\|Pure Rust vs FFI (pdfium / mupdf)]] | Accepted |
| 0002 | [[decisions/0002-lopdf-as-backend\|`lopdf` as the parsing backend]] | Accepted |
| 0003 | [[decisions/0003-pdfplumber-algorithm\|Replicate the pdfplumber algorithm]] | Accepted |
| 0004 | [[decisions/0004-mvp-scope\|MVP scope: no OCR, tables, or encryption]] | Accepted |
| 0005 | [[decisions/0005-error-type-design\|`Error` enum design]] | Accepted |
| 0006 | [[decisions/0006-cow-in-word-extraction\|`Cow<[Char]>` in `extract_words`]] | Accepted |
| 0007 | [[decisions/0007-lazy-char-cache\|Lazy per-page cache with `OnceLock`]] | Accepted |

## What an ADR looks like

Each one follows the same short shape:

- **Context**: what problem triggered the decision.
- **Decision**: what we did.
- **Consequences**: trade-offs accepted.
- **Alternatives considered**: what we rejected and why.

## When to open a new ADR

- The decision is hard to revert (touches public API or backend).
- It has trade-offs worth documenting for future maintainers.
- A PR review needed >1 round to reach consensus.

Minor changes (internal refactors, renames) **don't** need an ADR.
