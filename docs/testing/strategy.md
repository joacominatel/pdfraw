---
tags: [testing, strategy]
---

# Testing strategy

Four levels, each with its role:

```
       ┌────────────────────┐
       │   Doc tests        │   5 tests — runnable public-API examples
       └────────────────────┘
   ┌──────────────────────────┐
   │  Integration tests       │   15 tests — black-box end-to-end
   └──────────────────────────┘
┌──────────────────────────────────┐
│    Unit tests                    │   34 tests — internal modules
└──────────────────────────────────┘
        ┌──────────────────┐
        │ Snapshot tests   │   1 (inside integration) — layout output
        └──────────────────┘
```

**Total: 54 tests** running on every `cargo test`.

## Unit tests (`#[cfg(test)] mod tests` per module)

Tests with visibility into the module. One assertion per test, names
like `subject_behavior_when_condition`.

Locations:

| Module | Tests |
|--------|-------|
| `src/geom.rs` | 9 (matrix algebra, BBox merge) |
| `src/text/cluster.rs` | 5 (clustering edge and center cases) |
| `src/text/ligatures.rs` | 3 (FB01, FB03, no-op) |
| `src/text/extractor.rs` | 7 (WordExtractor rules + layout) |
| `src/parser/fonts/glyph_names.rs` | 4 (lookup + uniXXXX) |
| `src/parser/fonts/differences.rs` | 3 (override, increment, miss) |
| `src/parser/fonts/widths.rs` | 2 (array form, range form) |

## Integration tests (`tests/extraction.rs`)

Black-box end-to-end over real PDFs generated with `printpdf`.

| Test | What it covers |
|------|-----------|
| `opens_pdf_and_reports_page_count` | Document::from_bytes works |
| `extracts_chars_from_hello_world` | Basic char extraction |
| `extracts_chars_have_positive_coordinates` | Sanity of coords |
| `extract_chars_for_multiple_lines_yields_distinct_tops` | Multi-line |
| `extract_text_returns_visible_string` | extract_text |
| `extract_text_separates_lines_with_newline` | Line separation |
| `words_have_increasing_x_within_line` | Word order |
| `layout_preserves_invoice_columns` | Layout property: left/right cols |
| `layout_snapshot_invoice` | **snapshot** of the full output |
| `text_page_is_not_reported_as_scanned` | is_scanned false case |
| `page_metrics_match_a4` | width/height in pt |
| `multi_page_document_iterates_in_order` | Multi-page |
| `document_page_returns_out_of_bounds_when_index_too_large` | Error case |
| `document_open_reads_pdf_from_disk` | Document::open via tempfile |
| `text_options_builder_returns_overrides_when_fields_set` | Fluent builder |

## Doc tests

Examples in doc comments that compile and run with `cargo test --doc`:

| Location | What it validates |
|-----------|------------|
| `lib.rs` (crate-level) | The quick start from the README compiles |
| `Document::open` | Basic API |
| `Document::from_bytes` | Alternative API |
| `Page::chars` | Iterating over chars |
| `text::extractor::extract_words` | Direct call to the extractor |

## Snapshot tests with `insta`

For algorithms whose output is structured text, manual asserts get
fragile. `insta` stores a "golden" in `tests/snapshots/*.snap` and
diffs byte-for-byte on every run.

We only have one (deliberately — chapter 5.6 discourages huge
snapshots):

- `layout_snapshot_invoice` — exact output of `extract_text_layout`
  over a synthetic invoice.

When to add another:
- When we ship `extract_tables`.
- When we ship `Document::metadata`.
- When we add `to_markdown` or another serialization.

Workflow detail in [[testing/snapshot-workflow]].

## Conventions

### Naming

```rust
fn subject_behavior_when_condition() { ... }
fn function_returns_X_when_Y() { ... }
fn function_should_X_when_Y() { ... }
```

Examples from the crate:
- `layout_aligns_right_column_with_spaces` ✓
- `extract_type0_returns_indexed_widths_when_array_form` ✓
- `then_applies_self_first_then_other` ✓

### One assertion per test

```rust
// ❌ Don't
#[test]
fn validates_all_invariants() {
    assert!(c.x0 >= 0.0);
    assert!(c.x1 > c.x0);
    assert!(c.top >= 0.0);
}

// ✅ Do
#[test]
fn x0_is_non_negative() { assert!(c.x0 >= 0.0); }
#[test]
fn x1_is_greater_than_x0() { assert!(c.x1 > c.x0); }
#[test]
fn top_is_non_negative() { assert!(c.top >= 0.0); }
```

Exception: when the invariants are inseparably coupled (e.g. checks on
the same `Char` inside a loop over all chars), a `for + assert` is
acceptable. See `extracts_chars_have_positive_coordinates`.

### Fixtures via `tests/common/mod.rs`

Helpers that generate synthetic PDFs so we don't commit binaries:

```rust
common::build_pdf(&[(20.0, 270.0, "Hello")]);
common::build_two_page_pdf(&[...], &[...]);
```

## Commands

```bash
# Full pyramid
cargo test

# Unit tests only
cargo test --lib

# Integration only
cargo test --test extraction

# Doc tests only
cargo test --doc

# A specific test
cargo test --test extraction layout_snapshot_invoice

# With logging
RUST_LOG=debug cargo test -- --nocapture
```

## Quality gates

Before every commit:

```bash
cargo test                                              # all green
cargo clippy --all-targets --all-features -- -D warnings   # zero warnings
cargo fmt --check                                       # formatted
```

## Related reading

- [[testing/snapshot-workflow]]
- [Apollo Rust Best Practices — Chapter 5](https://github.com/apollographql/rust-best-practices/blob/main/chapter_05.md)
