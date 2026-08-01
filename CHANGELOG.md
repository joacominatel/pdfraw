# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While the crate is pre-1.0, behavioural changes may land in patch releases;
see the Status section of the README.

## [0.1.3] - 2026-08-01

Clears the entire known-defect backlog. The adversarial suite found 28
defects; 0.1.1 and 0.1.2 fixed 13, and this release fixes the remaining 15.
`rg '#\[ignore = "BUG' tests/` now returns nothing but the four file headers
describing the convention.

### Changed values on existing fields

No public signature changed, but `Char` reports different numbers in cases
where it was previously wrong. If you depend on exact coordinates, re-baseline
against this release.

- Glyphs on a **rotated page** are now in displayed coordinates rather than
  unrotated ones, and `Page::width`/`height` swap for a quarter turn.
- Glyphs on a page whose **`/MediaBox` does not start at `(0, 0)`** are no
  longer offset by that corner; a glyph inside the page could previously
  report a negative `top`.
- The glyph box now carries **`Tz` horizontal scaling**, which the advance
  already did.
- A **negative font size** no longer inverts the box: `x0 <= x1` and
  `top <= bottom` hold in all cases, as `Char` always documented.
- Fonts with **undefined codes or one-to-many `/ToUnicode` entries** now put
  each glyph on its own width instead of shifting every following glyph onto
  the previous code's.

### Fixed

- **`/Rotate` was reported but never applied**, so a landscape page came out
  sideways with nothing to indicate it. It is now composed as the outermost
  transform, and normalized to `{0, 90, 180, 270}`.
- **Text shown outside `BT`/`ET` was dropped** — and `is_scanned()` still
  counted the `Tj`, so the page reported neither text nor a need for OCR.
- **Byte codes and decoded characters were assumed to align index for
  index.** They do not: an encoding with no glyph for a code yields a shorter
  string, and a one-to-many `/ToUnicode` entry a longer one. Decoding is now
  per code, so the two cannot drift.
- **Word spacing was keyed on the decoded character**, so `Tw` was skipped
  entirely when `/Differences` remapped code 32 (PDF 32000-1 §9.3.3).
- **`/MediaBox` indirect numbers were not dereferenced**, so the page fell
  back to 612×792 and every glyph on it was mispositioned.
- **A zero-area `/MediaBox`** was accepted verbatim, while a *missing* one
  already fell back to a usable default.
- **`uXXXXX` glyph names were truncated to four hex digits**, so every
  supplementary-plane name decoded to an unrelated codepoint.
- **`Matrix::is_upright`** never checked the `d != 0` it documented, and
  floored its scale divisor at `1e-6` — which made any smaller matrix upright
  regardless of rotation.
- **An empty-text `Char` counted as blank** and split the word it sat inside.
- **`use_text_flow` only half worked**: it skipped the global sort and then
  sorted every line by `x0` anyway.
- **`Char::upright` was documented as a layout filter and never read**, so a
  sideways glyph joined whatever horizontal line it overlapped.
- **The two line-clustering stages disagreed.** `extract_words` chains within
  `y_tolerance`; `extract_text_layout` compared against the first word only,
  so a drifting baseline split differently in each. They now agree by
  construction.

### Added

- **CI.** Seven jobs on every pull request and on the long-lived branches:
  rustfmt, clippy with `-D warnings`, tests in debug *and* release, a build
  pinned to the declared MSRV, rustdoc with `-D warnings`, and a check that
  the published package carries no stray files. The last two are regression
  tests for mistakes this project already made — the yanked 0.1.1 and the
  local PDF corpus that nearly shipped.
- Dependabot for both the `github-actions` and `cargo` ecosystems.

## [0.1.2] - 2026-07-31

### Fixed

- **The declared minimum Rust version was wrong.** 0.1.1 shipped
  `rust-version = "1.85"`, but the `lopdf` 0.40 → 0.44 upgrade in that same
  release pulled in a dependency tree that cannot build below **1.88**:
  `lopdf` 0.44 declares `rust-version = "1.88"` itself and requires
  `weezl ^0.2`, which exists only at that floor. On Rust 1.85, 1.86 or 1.87
  the crate resolved as compatible and then failed to compile. The declared
  floor now matches reality.

  1.88+ toolchains were never affected.

### Changed

- Four nested `if let` chains collapsed into let-chains, which the corrected
  1.88 floor makes available. Behaviour is identical; this keeps `clippy`
  at zero warnings under the real minimum.

## [0.1.1] - 2026-07-31

### Security

Four defects let a PDF you did not write crash the parser or make it
allocate far more memory than the input justifies. `pdfraw` parses
untrusted files by design, so these are the sharpest items in this release.

- **A hostile `/FirstChar` overflowed `u32`.** `/Widths` computed
  `first_char + i` after casting `/FirstChar` straight to `u32`, so
  `4294967295` or `-1` panicked in debug and wrapped silently in release.
  A simple font is byte-indexed, so `/FirstChar` outside `0..=255` is now
  rejected and the addition is checked.
- **The layout grid was never clipped.** One glyph claiming `x0 = 1e8`
  turned a two-char page into 13,793,104 output characters, and a 55-byte
  content stream with a large `TJ` adjustment expanded to 2,758,623. Grid
  positions are now clamped to 20,000 rows and columns — roughly two
  orders of magnitude above what a real page needs.
- **An `x_density` or `y_density` of `0` divided by zero**, saturating the
  padding count to `i32::MAX` and attempting a ~2 GiB allocation.
  Non-finite and non-positive densities now fall back to the pdfplumber
  defaults with a warning.
- **NaN coordinates were laundered into ±infinity.** `f32::min`/`max`
  return the non-NaN operand, so folding from `INFINITY` turned an all-NaN
  word into `±inf`, which then saturated the layout grid. Non-finite
  values are now excluded rather than converted.

### Fixed

- **`/Rotate` was reported but never applied.** A quarter-turned page kept
  its unrotated dimensions and its glyphs stayed in unrotated coordinates,
  so a landscape scan came out sideways with no indication. The rotation is
  now composed into the transform stack as the outermost step, so page
  dimensions, glyph positions, `Char::size` and `Char::upright` all come out
  in displayed space. `/Rotate` is also normalized: negatives wrap into
  range, and a value that is not a multiple of 90 is reported as `0` with a
  warning instead of being passed through.
- **Text shown outside a `BT`/`ET` pair was silently dropped.** Show
  operators were gated on being inside a text object, so a content stream
  that omits `BT`, or closes it early, lost all of its text — and
  `is_scanned()` did not flag the page either, because it does count the
  `Tj`. The caller was told neither. Every reader that matters renders such
  text, so `pdfraw` now does too.

- **`Char::size` reported the matrix scale instead of the font size.** A 12pt
  font drawn under a 0.75 CTM came out as `0.75` rather than `9.0`, so every
  glyph on a real-world page shared one meaningless value. This also made
  `WordOptions::x_tolerance_ratio` (which multiplies by `Char::size`) useless.
  The effective size is now `Tf` size scaled by the text and current
  transformation matrices.
- **`Char::doctop` never accumulated preceding page heights.** It was set
  equal to `top` on every page, contradicting its documented meaning. Page
  heights are now summed once per document and applied as an offset.
- **`q`/`Q` did not save and restore the text state.** Only the CTM was
  stacked, so a font, size, character spacing, word spacing, horizontal
  scale, leading, or rise set inside a `q ... Q` block leaked out past the
  `Q`. All of these belong to the graphics state per PDF 32000-1, Table 52.
- **A cyclic `/Parent` chain hung the parser forever.** A malformed or
  hostile PDF whose page-tree nodes point at each other made the inheritable
  attribute lookup (`/MediaBox`, `/Rotate`) spin indefinitely. The walk is
  now capped at 64 levels.
- **A malformed `/W` array could exhaust memory.** A CID range naming
  billions of entries was expanded one map entry at a time. Ranges outside
  the 16-bit CID space, and descending ranges, are now rejected.
- **One unreadable glyph name discarded an entire `/Encoding /Differences`
  map.** The UTF-8 check used `?` inside the loop, returning `None` for the
  whole font and silently dropping every override already collected. Bad
  names are now skipped individually.
- **`Word::char_range` could be empty or wrong.** It recovered indices by
  scanning for pointer identity, which produced a reversed (and therefore
  empty) range whenever the positional sort reordered a word's glyphs
  relative to stream order. Ranges are now derived from the real indices,
  which also removes a quadratic scan over the char slice per word.

### Changed

- `lopdf` 0.40 → 0.44, `compact_str` 0.9 → 0.10.
- `Word::char_range` is documented as half-open (it always was a
  `Range<usize>`) and as potentially spanning glyphs that belong to other
  words, when the content stream emits them out of reading order.

### Added

- **A `pdfraw` command-line tool**, shipped from the same crate. Writes
  layout-preserving text to stdout, or to a file with `-o`, and reports
  skipped text-less pages on stderr instead of dropping them silently.
  Library consumers do not build it — Cargo skips a dependency's binaries.
- `tests/regression_fixes.rs` — one reproduction per defect above.
- `tests/adversarial_*.rs` — 144 tests written to break the library. The
  ones that succeed are parked behind `#[ignore = "BUG: ..."]`; grep for
  that string to read the inventory of known defects.
- This changelog.

## [0.1.0]

Initial release. See `docs/roadmap.md` for the feature inventory.
