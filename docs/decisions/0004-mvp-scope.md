---
tags: [adr, accepted, scope]
---

# ADR 0004 — MVP scope

**Status**: Accepted · **Date**: 2026-05-17

## Context

A "PDF extractor" crate could include many features: layout text,
tables, OCR, encryption, signing, annotations, forms. Each one is non-
trivially expensive.

The primary use case is **invoices and credit notes**: documents with
item tables, totals, sender/receiver data, and sometimes images (logos,
scanned pages).

## Decision

**The MVP covers only layout-preserving text extraction.**

Specifically:

| Feature | In MVP | Why |
|---------|--------|-------|
| Simple extract text (`Page::extract_text`) | ✅ | Baseline case |
| Layout extract text (`Page::extract_text_layout`) | ✅ | Core feature |
| `Char` / `Word` / `Page` model with coordinates | ✅ | Building blocks |
| Configurable `WordOptions` and `TextOptions` | ✅ | Tuning for weird PDFs |
| Scanned-page detection (`is_scanned`) | ✅ | Cheap, avoids confusion |
| Image OCR | ❌ | `feature = "ocr"` reserved, no code |
| Table detection (`extract_tables`) | ❌ | v0.2 |
| Password-encrypted PDFs | ❌ | `Error::Unsupported` for now |
| Editing / writing PDFs | ❌ | Out of scope for this crate |
| Annotations, signatures, forms | ❌ | Out of scope |

## Consequences

### ✅ What we gain

- **Focus**. ~3000 LOC well-tested instead of 10000 mediocre.
- **Short time to production**. The MVP covers ~80% of real vector
  invoice cases.
- **Stable public surface**. Less chance of future breaking changes.

### ❌ What we don't cover

- Scanned PDFs end as `Page::is_scanned() == true`; the caller decides
  whether to route to external OCR (Tesseract, AWS Textract, etc.).
- Invoices with complex tables require manual parsing with
  `Page::words()` + column logic in the caller.

## Post-MVP extension plan

`Cargo.toml` already reserves the feature flags:

```toml
[features]
default  = []
serde    = ["dep:serde", "compact_str/serde"]   # ✅ implemented
parallel = ["dep:rayon"]                        # ✅ reserved, no code
ocr      = []                                   # ⏳ placeholder for v0.2+
```

Proposed roadmap in [[roadmap]]:

1. **v0.2** — `extract_tables()` with `lines` and `text` strategies.
2. **v0.3** — opt-in OCR with `tesseract-rs`.
3. **v0.4** — `Document::open_with_password()`.

Each one with its own ADR when it arrives.

## Alternatives considered

### "Kitchen-sink" MVP

Also doing tables and OCR. Discarded due to time (we explored both as
agents and estimated +4 weeks each) and because it pushes the first
release too far out.

### "Plain text only" MVP

`extract_text()` without layout. Discarded because it loses the
differentiating value vs `pdftotext` and other wrappers.

## Related reading

- [[roadmap]]
- [[decisions/0003-pdfplumber-algorithm]]
