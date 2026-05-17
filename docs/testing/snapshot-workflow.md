---
tags: [testing, insta, snapshot]
---

# `insta` snapshot workflow

We use [`insta`](https://insta.rs) for snapshots of structural outputs,
mainly the result of `extract_text_layout`.

## Why snapshots

`extract_text_layout` returns a `String` that can have hundreds of
characters with precise spaces and newlines. Asserts like:

```rust
assert_eq!(output, "ACME                                ... Invoice\n\n\n\n...");
```

are fragile and horrible to read. `insta::assert_snapshot!(output)`
stores the expected text in `tests/snapshots/*.snap` and diffs on every
run.

## A typical test

```rust
#[test]
fn layout_snapshot_invoice() {
    let pdf = common::build_pdf(&[...]);
    let doc = Document::from_bytes(pdf).unwrap();
    let layout = doc.page(0).unwrap()
        .extract_text_layout(&TextOptions::pdfplumber_defaults())
        .unwrap();
    insta::assert_snapshot!(layout);
}
```

The snapshot name derives from the test name:
`tests/snapshots/extraction__layout_snapshot_invoice.snap`.

## When the snapshot fails

If you change the algorithm or a default (e.g. raise `x_density`), the
test will fail like this:

```
× extraction::layout_snapshot_invoice
  expected:
     ACME                                Invoice
     Line A                                              1.00
  actual:
     ACME                                      Invoice
     Line A                                                    1.00
```

And `insta` generates
`tests/snapshots/extraction__layout_snapshot_invoice.snap.new`.

## Resolution options

### Option 1 — `cargo-insta` CLI (recommended)

Once:

```bash
cargo install cargo-insta
```

Then:

```bash
cargo insta review     # interactive UI, accept/reject per snapshot
cargo insta accept     # accept all pending .snap.new
cargo insta reject     # discard them
```

### Option 2 — environment variable

```bash
INSTA_UPDATE=auto cargo test          # accept all automatically
INSTA_UPDATE=always cargo test        # even without changes
INSTA_UPDATE=no cargo test            # never update (default in CI)
```

### Option 3 — manual

```bash
mv tests/snapshots/extraction__X.snap.new tests/snapshots/extraction__X.snap
git diff tests/snapshots/                # review visually
git add tests/snapshots/extraction__X.snap
```

## Rules

1. **Snapshots are committed**. They live in git like any other code.
2. **Review before accepting**. `cargo insta review` shows the diff;
   pay attention because a wrongly accepted snapshot is a bug that
   passes silently.
3. **Don't snapshot huge objects**. Handbook chapter 5.6 discourages
   it: if your snapshot has 300 lines, split it into sub-snapshots.
4. **Don't snapshot primitives**. For `assert_eq!(x, 42)`, use
   `assert_eq!`, not a snapshot.
5. **Use descriptive names**:
   ```rust
   insta::assert_snapshot!("invoice_two_columns", layout);
   ```
   produces `extraction__invoice_two_columns.snap`, more readable than
   the default name.

## When the output has unstable bits

If your snapshot has dates, UUIDs, or paths that change, use
redactions:

```rust
insta::assert_yaml_snapshot!(data, {
    ".created_at" => "[timestamp]",
    ".id" => "[uuid]"
});
```

(Not applicable to this crate yet, but documented in case we get to
`Document::metadata()`.)

## Related reading

- [[testing/strategy]]
- [insta docs](https://insta.rs/docs/quickstart/)
- [[examples/invoice-snapshot]] — the snapshot we already have
