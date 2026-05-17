---
tags: [glossary, reference]
---

# PDF glossary

Terms that show up in both the code and the rest of this vault. If you
already know every term on this page, you can skip the "PDF basics"
section in [[architecture/overview]] and [[architecture/state-machine]].

## Coordinates

- **Point (pt)** — the native PDF unit. 1 pt = 1/72 inch. An A4 page is
  595.28 × 841.89 pt.
- **User space** — the space where positions are defined *before*
  applying transformations.
- **Bottom-up** — the PDF coordinate system: origin at the **bottom-left**
  corner of the page. The crate flips everything to **top-down** (origin
  at the top-left) before exposing it to users, the same convention as
  `pdfplumber` (Python).
- **MediaBox** — `[x0 y0 x1 y1]` rectangle defining the page size, in pt.
  Inheritable from the `/Pages` ancestor.

## Content stream and operators

- **Content stream** — the binary sequence of operators that paints a
  page. Decoded by `lopdf::content::Content::decode`.
- **`BT` / `ET`** — *Begin Text* / *End Text*. Delimit text blocks.
- **`Tj`, `TJ`, `'`, `"`** — "show text" operators. `TJ` takes an array
  that mixes strings with numeric kerning.
- **`Tm`, `Td`, `TD`, `T*`** — text positioning (set matrix, translate,
  translate + leading, next line).
- **`Tf`, `Tc`, `Tw`, `Tz`, `TL`, `Ts`** — text state: font + size,
  char/word spacing, horizontal scaling, leading, rise.
- **`q` / `Q`** — push / pop the graphics state stack.
- **`cm`** — concatenate matrix (mutates the current CTM).
- **`Do`** — invoke an XObject (image, form). Used by
  [[architecture/overview]] for scanned-page detection.

Full table in [[reference/pdf-operators-supported]].

## Matrices

- **CTM** — *Current Transformation Matrix*. 3×3 matrix mapping user
  space → device space. Mutated by `cm`.
- **Tm** — *Text Matrix*. Position + scale/rotation of the text cursor.
  Reset to identity at every `BT`.
- **Tlm** — *Text Line Matrix*. Remembers where the current line
  started, so `T*` and `'` can move "to the next row".

## Fonts

- **Encoding** — a `byte → character` table. PDF defines several
  predefined ones (`WinAnsiEncoding`, `MacRomanEncoding`,
  `StandardEncoding`, `MacExpertEncoding`, `PDFDocEncoding`) and allows
  custom.
- **`/Differences`** — array inside an `Encoding` dict that overrides
  specific codes with a different glyph name. Covered in
  [[reference/font-encodings]].
- **CMap** — *Character Map*. Multibyte-to-Unicode table used in Type0
  (CID) fonts.
- **ToUnicode CMap** — PDF stream that provides the `glyph → Unicode`
  mapping. When present and complete, guarantees correct text extraction.
- **Type0 (CIDFont)** — composite, multibyte font. Uses
  `DescendantFonts[0]` for widths (`/W` array).
- **Adobe Glyph List** — dictionary mapping PostScript names
  (`/copyright`, `/Euro`) to Unicode codepoints. We have a subset in
  `src/parser/fonts/glyph_names.rs`.

## Layout algorithm

- **`x_tolerance` / `y_tolerance`** — maximum gap between glyphs for
  them to count as part of the same word / line.
- **`x_density` / `y_density`** — pt per column / line in the output's
  virtual monospaced grid. Defaults `7.25 / 13.0`.
- **1D cluster** — group sorted values while each is within the
  tolerance of the last in the group. See
  [[architecture/layout-algorithm]].
- **WordExtractor** — pass that turns chars into words by applying
  backtrack / gap / line-break rules from pdfplumber.
- **TextMap** — vector of `(out_char, Option<&Char>)` that maps each
  output character back to the PDF glyph that produced it. Useful for
  visual highlight.

## Crate types

- **[[reference/api|Document]]** — owns the parsed PDF.
- **Page** — view of a specific page, with a lazy chars cache.
- **Char** — a glyph positioned in top-down coords.
- **Word** — cluster of contiguous chars on a line.
- **TextOptions** — knobs for the layout extractor.
