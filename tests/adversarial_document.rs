//! Adversarial tests for document- and page-level metadata: `/MediaBox`,
//! `/Rotate`, page trees, and malformed input bytes.
//!
//! Tests marked `#[ignore = "BUG: ..."]` reproduce a real defect.

mod synthetic;

use lopdf::{Object, dictionary};
use pdfraw::prelude::*;
use synthetic::{PdfBuilder, page_chars, simple_font, two_page_doc};

const TEXT: &str = "BT /F1 10 Tf 100 700 Td <41> Tj ET";

fn page_with_media_box(media_box: Vec<Object>) -> Document {
    PdfBuilder::new()
        .font("F1", simple_font("WinAnsiEncoding", 65, &[1000, 1000]))
        .content(TEXT)
        .media_box(media_box)
        .build()
}

// ---------------------------------------------------------------------------
// /MediaBox
// ---------------------------------------------------------------------------

#[test]
fn media_box_with_reversed_corners_yields_positive_dimensions() {
    let doc = page_with_media_box(vec![612.into(), 792.into(), 0.into(), 0.into()]);
    let page = doc.page(0).unwrap();
    assert!((page.width() - 612.0).abs() < 0.5, "width {}", page.width());
    assert!(
        (page.height() - 792.0).abs() < 0.5,
        "height {}",
        page.height()
    );
}

#[test]
fn media_box_with_real_values_is_accepted() {
    let doc = page_with_media_box(vec![
        Object::Real(0.0),
        Object::Real(0.0),
        Object::Real(595.276),
        Object::Real(841.89),
    ]);
    let page = doc.page(0).unwrap();
    assert!(
        (page.width() - 595.276).abs() < 0.1,
        "width {}",
        page.width()
    );
}

#[test]
fn media_box_with_indirect_numbers_is_resolved() {
    let mut builder = PdfBuilder::new().font("F1", simple_font("WinAnsiEncoding", 65, &[1000]));
    let w = builder.add_object(Object::Integer(595));
    let h = builder.add_object(Object::Integer(842));
    let doc = builder
        .content(TEXT)
        .media_box(vec![
            0.into(),
            0.into(),
            Object::Reference(w),
            Object::Reference(h),
        ])
        .build();
    let page = doc.page(0).unwrap();
    assert!(
        (page.width() - 595.0).abs() < 0.5,
        "indirect /MediaBox bounds were not resolved: width {}",
        page.width()
    );
}

#[test]
fn media_box_with_zero_area_falls_back_to_a_usable_page() {
    let doc = page_with_media_box(vec![0.into(), 0.into(), 0.into(), 0.into()]);
    let page = doc.page(0).unwrap();
    assert!(
        page.width() > 0.0 && page.height() > 0.0,
        "degenerate page: {}x{}",
        page.width(),
        page.height()
    );
}

#[test]
fn char_top_is_measured_from_the_media_box_origin() {
    let doc = PdfBuilder::new()
        .font("F1", simple_font("WinAnsiEncoding", 65, &[1000]))
        .content("BT /F1 10 Tf 100 900 Td <41> Tj ET")
        .media_box(vec![0.into(), 100.into(), 595.into(), 942.into()])
        .build();
    let chars = page_chars(&doc);
    assert_eq!(chars.len(), 1);
    assert!(
        chars[0].top >= 0.0,
        "glyph inside the page box reported top={}",
        chars[0].top
    );
}

#[test]
fn missing_media_box_falls_back_to_letter_size() {
    let doc = PdfBuilder::new()
        .font("F1", simple_font("WinAnsiEncoding", 65, &[1000]))
        .content(TEXT)
        .page_entry("MediaBox", Object::Null)
        .build();
    let page = doc.page(0).unwrap();
    assert!(page.width() > 0.0 && page.height() > 0.0);
}

#[test]
fn media_box_inherited_from_the_pages_node_is_used() {
    let doc = PdfBuilder::new()
        .font("F1", simple_font("WinAnsiEncoding", 65, &[1000]))
        .content(TEXT)
        .pages_entry("MediaBox", vec![0.into(), 0.into(), 300.into(), 400.into()])
        .build();
    let page = doc.page(0).unwrap();
    assert!(
        (page.width() - 300.0).abs() < 0.5,
        "inherited /MediaBox ignored: width {}",
        page.width()
    );
}

#[test]
fn media_box_with_wrong_arity_falls_back_to_a_usable_page() {
    let doc = page_with_media_box(vec![0.into(), 0.into(), 595.into(), 842.into(), 99.into()]);
    let page = doc.page(0).unwrap();
    assert!(page.width() > 0.0 && page.height() > 0.0);
}

#[test]
fn media_box_of_the_wrong_type_falls_back_to_a_usable_page() {
    let doc = PdfBuilder::new()
        .content(TEXT)
        .page_entry("MediaBox", Object::string_literal("nonsense"))
        .build();
    let page = doc.page(0).unwrap();
    assert!(page.width() > 0.0 && page.height() > 0.0);
}

#[test]
fn media_box_with_a_dangling_reference_falls_back_to_a_usable_page() {
    let doc = PdfBuilder::new()
        .content(TEXT)
        .page_entry("MediaBox", Object::Reference((9999, 0)))
        .build();
    let page = doc.page(0).unwrap();
    assert!(page.width() > 0.0 && page.height() > 0.0);
}

// ---------------------------------------------------------------------------
// /Rotate
// ---------------------------------------------------------------------------

#[test]
fn rotate_inherited_through_an_indirect_reference_is_read() {
    let mut builder = PdfBuilder::new().content(TEXT);
    let rotate = builder.add_object(Object::Integer(180));
    let doc = builder
        .pages_entry("Rotate", Object::Reference(rotate))
        .build();
    assert_eq!(doc.page(0).unwrap().rotation(), 180);
}

#[test]
fn page_dimensions_swap_when_rotate_is_ninety() {
    let doc = PdfBuilder::new()
        .font("F1", simple_font("WinAnsiEncoding", 65, &[1000]))
        .content(TEXT)
        .media_box(vec![0.into(), 0.into(), 595.into(), 842.into()])
        .page_entry("Rotate", 90)
        .build();
    let page = doc.page(0).unwrap();
    assert!(
        (page.width() - 842.0).abs() < 0.5,
        "rotated page reports width {} (expected the swapped 842)",
        page.width()
    );
}

#[test]
fn rotation_is_normalized_when_rotate_is_not_a_quarter_turn() {
    let doc = PdfBuilder::new()
        .content(TEXT)
        .page_entry("Rotate", 45)
        .build();
    let r = doc.page(0).unwrap().rotation();
    assert!(
        matches!(r, 0 | 90 | 180 | 270),
        "rotation() returned {r}, outside the documented set"
    );
}

#[test]
fn rotation_is_normalized_when_rotate_is_negative() {
    let doc = PdfBuilder::new()
        .content(TEXT)
        .page_entry("Rotate", -90)
        .build();
    let r = doc.page(0).unwrap().rotation();
    assert_eq!(r, 270, "negative /Rotate was not normalized");
}

// ---------------------------------------------------------------------------
// Cross-page state
// ---------------------------------------------------------------------------

#[test]
fn doctop_accumulates_the_height_of_previous_pages() {
    let doc = two_page_doc(TEXT, TEXT, 595, 842);
    let p0 = doc.page(0).unwrap();
    let p1 = doc.page(1).unwrap();
    let c0 = p0.chars().unwrap()[0].clone();
    let c1 = p1.chars().unwrap()[0].clone();
    assert!(
        (c1.doctop - (c1.top + 842.0)).abs() < 1.0,
        "page-1 glyph has doctop={} with top={} on an 842pt page",
        c1.doctop,
        c1.top
    );
    assert!(
        c1.doctop > c0.doctop,
        "glyphs on different pages share the same doctop"
    );
}

// ---------------------------------------------------------------------------
// Broken documents
// ---------------------------------------------------------------------------

#[test]
fn from_bytes_returns_error_for_empty_input() {
    assert!(Document::from_bytes(Vec::new()).is_err());
}

#[test]
fn from_bytes_does_not_panic_on_garbage() {
    let garbage: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
    if let Ok(doc) = Document::from_bytes(garbage) {
        assert_eq!(doc.num_pages(), 0, "garbage produced pages");
    }
}

#[test]
fn from_bytes_does_not_panic_on_a_truncated_pdf() {
    let bytes = PdfBuilder::new()
        .font("F1", simple_font("WinAnsiEncoding", 65, &[1000]))
        .content(TEXT)
        .into_bytes();
    let half = bytes.len() / 2;
    if let Ok(doc) = Document::from_bytes(&bytes[..half]) {
        for page in doc.pages().flatten() {
            let _ = page.chars();
        }
    }
}

#[test]
fn page_out_of_bounds_is_reported_for_usize_max() {
    let doc = PdfBuilder::new().content(TEXT).build();
    assert!(matches!(
        doc.page(usize::MAX),
        Err(Error::PageOutOfBounds(_))
    ));
}

#[test]
fn document_with_an_empty_page_tree_reports_no_pages() {
    let mut ldoc = lopdf::Document::with_version("1.7");
    let pages_id = ldoc.new_object_id();
    ldoc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => Vec::<Object>::new(),
            "Count" => 0,
        }),
    );
    let catalog_id = ldoc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    ldoc.trailer.set("Root", catalog_id);
    let mut buf = Vec::new();
    ldoc.save_to(&mut buf).unwrap();

    let doc = Document::from_bytes(buf).expect("empty page tree should still load");
    assert_eq!(doc.num_pages(), 0);
    assert!(doc.page(0).is_err());
}

#[test]
fn page_without_contents_yields_no_chars() {
    let doc = PdfBuilder::new()
        .content(TEXT)
        .page_entry("Contents", Object::Null)
        .build();
    let chars = page_chars(&doc);
    assert!(chars.is_empty());
}

#[test]
fn page_without_resources_still_extracts_text() {
    let doc = PdfBuilder::new().without_resources().content(TEXT).build();
    let chars = page_chars(&doc);
    assert_eq!(chars.len(), 1, "text lost when /Resources is absent");
}

#[test]
fn is_scanned_agrees_with_char_extraction_when_bt_is_missing() {
    let doc = PdfBuilder::new()
        .font("F1", simple_font("WinAnsiEncoding", 65, &[1000]))
        .content("/F1 10 Tf 100 700 Td <41> Tj")
        .build();
    let page = doc.page(0).unwrap();
    let has_chars = !page.chars().unwrap().is_empty();
    let scanned = page.is_scanned().unwrap();
    assert!(
        has_chars || scanned,
        "page yields no chars yet is_scanned() == false"
    );
}
