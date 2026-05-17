---
tags: [vault-root, pdfraw]
---

# pdfraw — Vault

Living documentation for the `pdfraw` crate. Designed to be opened
as an **Obsidian Vault**: every Markdown file lives in its semantic
folder and links to the rest via `[[wikilinks]]`.

> If this is your first time here, read in this order:
> 1. [[guides/quick-start]]
> 2. [[architecture/overview]]
> 3. [[reference/api]]
> 4. If you want to understand *why* something was built the way it is,
>    the [[decisions/index|ADRs]].

## Vault map

```
docs/
├── README.md                          ← you are here (vault entrypoint)
├── index.md                           ← map with links to everything
├── glossary.md                        ← PDF vocabulary used by the crate
├── roadmap.md                         ← what's missing and what's next
├── architecture/
│   ├── overview.md
│   ├── module-map.md
│   ├── state-machine.md
│   └── layout-algorithm.md
├── decisions/                         ← numbered ADRs
│   ├── index.md
│   ├── 0001-pure-rust-vs-ffi.md
│   ├── 0002-lopdf-as-backend.md
│   ├── 0003-pdfplumber-algorithm.md
│   ├── 0004-mvp-scope.md
│   ├── 0005-error-type-design.md
│   ├── 0006-cow-in-word-extraction.md
│   └── 0007-lazy-char-cache.md
├── guides/
│   ├── quick-start.md
│   ├── extracting-invoices.md
│   └── tuning-text-options.md
├── reference/
│   ├── api.md
│   ├── pdf-operators-supported.md
│   └── font-encodings.md
├── examples/
│   ├── cli-extract.md
│   └── invoice-snapshot.md
└── testing/
    ├── strategy.md
    └── snapshot-workflow.md
```

## Where to start by role

| If you are… | Start here |
|------|------------------|
| A crate user | [[guides/quick-start]], [[reference/api]] |
| A new contributor | [[architecture/overview]], [[architecture/module-map]] |
| A technical reviewer | [[decisions/index]], [[architecture/state-machine]] |
| Curious about the pdfplumber algorithm | [[architecture/layout-algorithm]] |

## How to work with this vault

- Open the `docs/` folder as a vault in Obsidian.
- Wikilinks `[[file]]` and `[[folder/file|alias]]` work natively.
- Frontmatter is optional; the `tags:` header on each note is for
  Obsidian's tag pane only.
- Images and diagrams go next to the `.md` that uses them.

## Writing conventions

- **ADRs** are immutable once accepted. If a decision changes, open a
  new ADR that supersedes the old one and link them with
  `[[…|supersedes]]`.
- **Guides** are step-by-step practical tutorials.
- **Reference** documents the public surface of the crate and stays in
  sync with rustdoc.
- **Architecture** documents the *how* internally; it changes with
  implementation.
- In Rust code, prefer fenced `rust` blocks so Obsidian highlights them.

## Links outside the vault

- Source: `../src/`
- Local rustdoc: `cargo doc --open --no-deps -p pdfraw`
- Original implementation plan: `~/.claude/plans/el-objetivo-de-este-replicated-canyon.md`
