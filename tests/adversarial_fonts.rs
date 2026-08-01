//! Adversarial tests for font decoding: encodings, `/Differences`,
//! `/Widths`, and composite (Type0) fonts.
//!
//! Tests marked `#[ignore = "BUG: ..."]` reproduce a real defect.

mod synthetic;

use lopdf::{Dictionary, Object, Stream, dictionary};
use pdfraw::prelude::*;
use synthetic::{PdfBuilder, page_chars, simple_font};

fn chars_with_font(font: Dictionary, content: &str) -> Vec<Char> {
    let doc = PdfBuilder::new().font("F1", font).content(content).build();
    page_chars(&doc)
}

// ---------------------------------------------------------------------------
// Byte-code ↔ decoded-char alignment
// ---------------------------------------------------------------------------

#[test]
#[ignore = "BUG: emit_string assumes decoded.chars() aligns index-for-index with the byte codes; an undefined code is dropped by the encoding table, shifting every following glyph onto the wrong width"]
fn glyph_width_uses_its_own_code_when_an_undefined_code_precedes_it() {
    // 0xB0 is undefined in StandardEncoding, so the decoder drops it.
    let mut font = simple_font("StandardEncoding", 65, &[1000, 1000]);
    font.set("MissingWidth", 100);
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <41B042> Tj ET");

    let text: String = chars.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text, "AB", "unexpected decoding: {text:?}");
    let b = &chars[1];
    assert!(
        (b.x1 - b.x0 - 10.0).abs() < 0.5,
        "glyph 'B' took the width of the dropped code: box width {}",
        b.x1 - b.x0
    );
}

#[test]
#[ignore = "BUG: a one-to-many ToUnicode mapping makes decoded.chars() longer than the code list; the extra chars fall back to code 0 and add a phantom advance"]
fn one_to_many_cmap_entry_advances_only_once() {
    let mut builder = PdfBuilder::new();
    let to_unicode = builder.add_object(Stream::new(dictionary! {}, cmap_with_ligature()));
    let cid_font = builder.add_object(Object::Dictionary(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "Test",
        "W" => vec![
            Object::Integer(0x41),
            Object::Array(vec![Object::Integer(1000)]),
            Object::Integer(0x42),
            Object::Array(vec![Object::Integer(1000)]),
        ],
    }));
    let font = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "Test",
        "Encoding" => "Identity-H",
        "DescendantFonts" => vec![Object::Reference(cid_font)],
        "ToUnicode" => Object::Reference(to_unicode),
    };
    let doc = builder
        .font("F1", font)
        .content("BT /F1 10 Tf 100 700 Td <00410042> Tj ET")
        .build();
    let chars = page_chars(&doc);

    let text: String = chars.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text, "fiB", "unexpected decoding: {text:?}");
    let last = chars.last().unwrap();
    assert!(
        (last.x0 - 110.0).abs() < 0.5,
        "the two-char ligature advanced twice: 'B' landed at x0={}",
        last.x0
    );
}

/// A minimal `ToUnicode` CMap where CID 0x41 expands to "fi".
fn cmap_with_ligature() -> Vec<u8> {
    let lines = [
        "/CIDInit /ProcSet findresource begin",
        "12 dict begin",
        "begincmap",
        "/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def",
        "/CMapName /Adobe-Identity-UCS def",
        "/CMapType 2 def",
        "1 begincodespacerange",
        "<0000> <FFFF>",
        "endcodespacerange",
        "2 beginbfchar",
        "<0041> <00660069>",
        "<0042> <0042>",
        "endbfchar",
        "endcmap",
        "CMapName currentdict /CMap defineresource pop",
        "end",
        "end",
    ];
    lines.join("\n").into_bytes()
}

#[test]
fn identity_h_font_without_to_unicode_emits_one_glyph_per_cid() {
    let mut builder = PdfBuilder::new();
    let cid_font = builder.add_object(Object::Dictionary(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "Test",
        "W" => vec![
            Object::Integer(0x41),
            Object::Array(vec![Object::Integer(1000), Object::Integer(1000)]),
        ],
    }));
    let font = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "Test",
        "Encoding" => "Identity-H",
        "DescendantFonts" => vec![Object::Reference(cid_font)],
    };
    let doc = builder
        .font("F1", font)
        .content("BT /F1 10 Tf 100 700 Td <00410042> Tj ET")
        .build();
    let chars = page_chars(&doc);

    assert_eq!(chars.len(), 2, "expected two glyphs, got {chars:?}");
    assert!(
        (chars[0].x0 - 100.0).abs() < 0.5,
        "first CID was placed at x0={} instead of the Td origin",
        chars[0].x0
    );
    assert!(
        (chars[1].x0 - 110.0).abs() < 0.5,
        "second CID was placed at x0={} instead of one em further",
        chars[1].x0
    );
}

// ---------------------------------------------------------------------------
// /Widths arithmetic
// ---------------------------------------------------------------------------

#[test]
fn widths_with_first_char_at_u32_max_does_not_overflow() {
    let font = simple_font("WinAnsiEncoding", 4_294_967_295, &[500, 500]);
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
}

#[test]
fn widths_with_negative_first_char_does_not_overflow() {
    let font = simple_font("WinAnsiEncoding", -1, &[500, 500]);
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
}

#[test]
fn widths_with_non_numeric_entries_does_not_panic() {
    let mut font = simple_font("WinAnsiEncoding", 65, &[1000]);
    font.set(
        "Widths",
        vec![
            Object::Integer(1000),
            Object::Name(b"bogus".to_vec()),
            Object::Null,
        ],
    );
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <414243> Tj ET");
    assert_eq!(chars.len(), 3);
}

#[test]
fn type0_widths_with_truncated_pairs_does_not_panic() {
    let mut builder = PdfBuilder::new();
    let cid_font = builder.add_object(Object::Dictionary(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "Test",
        // Trailing CID with no width, and a reversed range.
        "W" => vec![
            Object::Integer(30),
            Object::Integer(10),
            Object::Integer(500),
            Object::Integer(99),
        ],
    }));
    let font = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "Test",
        "Encoding" => "Identity-H",
        "DescendantFonts" => vec![Object::Reference(cid_font)],
    };
    let doc = builder
        .font("F1", font)
        .content("BT /F1 10 Tf 100 700 Td <0041> Tj ET")
        .build();
    let page = doc.page(0).unwrap();
    assert!(page.chars().is_ok());
}

// ---------------------------------------------------------------------------
// /Encoding /Differences
// ---------------------------------------------------------------------------

fn font_with_differences(diffs: Vec<Object>) -> Dictionary {
    let mut font = simple_font("", 65, &[1000, 1000]);
    font.set(
        "Encoding",
        dictionary! {
            "Type" => "Encoding",
            "BaseEncoding" => "WinAnsiEncoding",
            "Differences" => diffs,
        },
    );
    font
}

#[test]
fn differences_with_a_supplementary_plane_uni_name_decodes_fully() {
    let font = font_with_differences(vec![Object::Integer(65), Object::Name(b"u1F600".to_vec())]);
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
    assert_eq!(
        chars[0].text.as_str(),
        "\u{1F600}",
        "supplementary-plane glyph name was truncated"
    );
}

#[test]
fn differences_with_out_of_range_codes_does_not_shift_the_rest() {
    let font = font_with_differences(vec![
        Object::Integer(300),
        Object::Name(b"bullet".to_vec()),
        Object::Integer(65),
        Object::Name(b"copyright".to_vec()),
    ]);
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
    assert_eq!(chars[0].text.as_str(), "©");
}

#[test]
fn differences_with_an_unknown_glyph_name_falls_back_to_the_base_encoding() {
    let font = font_with_differences(vec![
        Object::Integer(65),
        Object::Name(b"notarealglyphname".to_vec()),
    ]);
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
    assert_eq!(
        chars[0].text.as_str(),
        "A",
        "an unknown glyph name must not destroy the base decoding"
    );
}

#[test]
fn differences_with_a_surrogate_uni_name_falls_back_to_the_base_encoding() {
    // U+D800 is an unpaired surrogate and has no `char` representation.
    let font = font_with_differences(vec![Object::Integer(65), Object::Name(b"uniD800".to_vec())]);
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
    assert_eq!(chars[0].text.as_str(), "A");
}

#[test]
fn differences_with_a_non_latin_uni_name_round_trips() {
    // U+05D0 HEBREW LETTER ALEF — right-to-left, outside Latin-1.
    let font = font_with_differences(vec![Object::Integer(65), Object::Name(b"uni05D0".to_vec())]);
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <41> Tj ET");
    assert_eq!(chars.len(), 1);
    assert_eq!(chars[0].text.as_str(), "\u{05D0}");
}

#[test]
fn empty_show_operators_produce_no_chars() {
    let font = simple_font("WinAnsiEncoding", 65, &[1000]);
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <> Tj [] TJ ET");
    assert!(chars.is_empty(), "empty strings produced {chars:?}");
}

#[test]
fn font_dictionary_without_type_entry_still_decodes_text() {
    let mut font = simple_font("WinAnsiEncoding", 65, &[1000, 1000]);
    font.remove(b"Type");
    let chars = chars_with_font(font, "BT /F1 10 Tf 100 700 Td <4142> Tj ET");
    let text: String = chars.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text, "AB");
}
