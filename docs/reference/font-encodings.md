---
tags: [reference, fonts, encoding]
---

# Font encodings

How the crate turns the bytes from a `Tj` into Unicode chars. This is
the trickiest area because real PDFs use ~15 different encoding
strategies.

## Decoding cascade

For every `Tj`/`TJ`, inside `emit_string`:

```
bytes ──┬──► /Encoding /Differences override (byte-by-byte)
        │       │
        │       └─► hit  ──► use override char
        │       └─► miss ──► continue
        │
        └──► lopdf::Encoding::bytes_to_string()
                │
                ├──► OneByteEncoding (WinAnsi/MacRoman/Standard/MacExpert/PDFDocEncoding)
                ├──► SimpleEncoding (Identity-H/V with /ToUnicode CMap)
                ├──► UnicodeMapEncoding (custom parsed CMap)
                └──► Latin-1 fallback if everything else fails
```

## OneByteEncoding (simple fonts)

Simple PDFs define their font as:

```pdf
<<
  /Type /Font
  /Subtype /Type1
  /BaseFont /Helvetica
  /Encoding /WinAnsiEncoding
>>
```

`lopdf` has the five predefined tables as `static [Option<u16>; 256]`.
Decoding is a direct lookup:

```
byte 0x41 ──► Some(0x0041) ──► 'A'
byte 0x80 ──► None         ──► (omit or replacement char)
```

| Encoding | Covers |
|----------|-------|
| `StandardEncoding` | Type1 base 14 fonts |
| `WinAnsiEncoding` | Windows-1252-ish, the most common |
| `MacRomanEncoding` | Mac, rare today |
| `MacExpertEncoding` | Mac expert characters |
| `PDFDocEncoding` | Metadata, rarely for fonts |

## `/Encoding /Differences`

When the font needs a different mapping for specific codes:

```pdf
<<
  /Type /Encoding
  /BaseEncoding /WinAnsiEncoding
  /Differences [
    96  /quoteleft
    169 /copyright
    174 /registered
    240 /Euro
  ]
>>
```

This overrides byte `0x60` to map to `'\u{2018}'` even though base
WinAnsi says otherwise.

`lopdf` 0.40 **does not implement Differences** (its own source comment
says so). We do, in `src/parser/fonts/differences.rs`:

```rust
let diffs = HashMap<u8, char>;   // 0x60 → '‘', 0xA9 → '©', ...
```

If a byte falls in the override map, we use its char. Otherwise we fall
to the base encoding.

### How `Differences` is parsed

The array alternates numbers and names:
`[ start_code, name, name, ..., new_code, name, ... ]`. A number resets
the position; each following name increments the code by 1.

```
[32, /space, /exclam, /quotedbl, 169, /copyright]
       ↓        ↓         ↓             ↓
      32       33        34            169
```

## Type0 (composite / CIDFont)

Fonts with many glyphs (e.g. CJK, fonts with thousands of Unicode
variants) use Type0:

```pdf
<<
  /Type /Font
  /Subtype /Type0
  /BaseFont /MyFont
  /Encoding /Identity-H
  /DescendantFonts [ <<
      /Type /Font
      /Subtype /CIDFontType2
      /W [ 0 [ 600 ] 32 38 700 ... ]
  >> ]
  /ToUnicode <<stream...>>
>>
```

Decoding:

1. `Identity-H` means: every 2 bytes = 1 CID (multibyte).
2. The `ToUnicode` CMap converts CID → Unicode (usually UTF-16BE).
3. `/W` array provides widths per CID.

`lopdf::Encoding::UnicodeMapEncoding` handles this transparently. We
only parse the `/W` (lopdf does not expose it) in
`src/parser/fonts/widths.rs`.

### Two forms of the `/W` array

```
[ first_cid [w1 w2 w3 ...] ]   // array form
[ first_cid last_cid w ]        // range form
```

Both forms can be mixed in the same array. See
[[architecture/state-machine|state-machine#how-we-decode-bytes-to-unicode]]
for the code.

## ToUnicode CMap

The `/ToUnicode` stream contains a CMap with `bfchar`/`bfrange`
sections:

```
2 beginbfchar
<0041> <0041>      % CID 65 → U+0041 'A'
<0042> <0042>      % CID 66 → U+0042 'B'
endbfchar

1 beginbfrange
<0030> <0039> <0030>   % CIDs 48..57 → '0'..'9'
endbfrange
```

`lopdf` parses this internally. We don't expose its `ToUnicodeCMap`
(it's private in lopdf 0.40); we use
`Encoding::UnicodeMapEncoding(cmap)`, which does `bytes_to_string`
returning a `String`.

## When decoding fails

Indicators that something went wrong:

| Symptom | Likely cause | Workaround |
|---------|----------------|------------|
| `\u{0001}`, `\u{0002}` in the output | Font without `/ToUnicode` | No crate-level fix; report the PDF |
| Text in "weird" characters (boxes) | Encoding wrongly applied | Inspect `c.fontname` and the dict encoding |
| Lost accents (`á` → `a`) | `Differences` with unsupported names | Add the name to the table in `glyph_names.rs` |
| CJK as raw bytes | Incomplete ToUnicode | Report; no crate-side patch |

## Adobe Glyph List subset

`src/parser/fonts/glyph_names.rs` has ~150 entries covering:

- Printable ASCII and signs.
- Latin-1 supplement (À-ÿ).
- Common typographic glyphs (em/en dash, smart quotes, ellipsis,
  bullet).
- Commercial symbols (€, ©, ®, ™, ¥, £, ¢).
- The 7 ligatures (`ff`, `fi`, `fl`, `ffi`, `ffl`, `ft`, `st`).
- `uniXXXX` and `uXXXX` names parsed dynamically.

If you find a PDF using a name outside this list, add it to the const
table with its matching Unicode codepoint.

## Related reading

- [[architecture/state-machine]]
- [[reference/pdf-operators-supported]]
- PDF 1.7 spec, section 9 ("Text")
