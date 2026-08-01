# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While the crate is pre-1.0, behavioural changes may land in patch releases;
see the Status section of the README.

## [0.1.1] - 2026-07-31

### Fixed

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

- `tests/regression_fixes.rs` — one reproduction per defect above.
- This changelog.

## [0.1.0]

Initial release. See `docs/roadmap.md` for the feature inventory.
