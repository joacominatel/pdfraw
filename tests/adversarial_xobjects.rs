//! Form XObject execution.
//!
//! A page can delegate part of its drawing to a Form XObject and invoke it
//! with `Do`. Text drawn that way is text on the page: every reader shows it,
//! and pdfplumber extracts it. Skipping the operator loses it with no error
//! and no warning, which is the one failure mode this crate cannot have.
//!
//! These tests also pin the hostile cases. A form may name itself, or two
//! forms may name each other, and the file is under no obligation to be
//! honest about it.

mod synthetic;

use lopdf::{Object, dictionary};
use pdfraw::TextOptions;
use synthetic::{PdfBuilder, simple_font, with_timeout};

/// `/F1` with a 500/1000 advance for every printable byte, so positions in
/// these tests are arithmetic rather than font-dependent.
fn font() -> lopdf::Dictionary {
    simple_font("WinAnsiEncoding", 32, &[500; 96])
}

fn font_resources() -> lopdf::Dictionary {
    dictionary! { "Font" => dictionary! { "F1" => font() } }
}

/// Collect the text of every glyph on page 0, in stream order.
fn text_of(doc: &pdfraw::Document) -> String {
    let page = doc.page(0).expect("page 0");
    page.chars()
        .expect("chars")
        .iter()
        .map(|c| c.text.as_str())
        .collect()
}

#[test]
fn text_inside_a_form_xobject_is_extracted() {
    let mut b = PdfBuilder::new();
    let form = b.add_form(
        "BT /F1 12 Tf 100 700 Td (HELLO) Tj ET",
        dictionary! { "Resources" => font_resources() },
    );
    let doc = b.xobject("Fm1", form).content("/Fm1 Do").build();

    assert_eq!(
        text_of(&doc),
        "HELLO",
        "text drawn inside a Form XObject must reach chars()"
    );
}

#[test]
fn form_xobject_text_reaches_the_layout_output() {
    let mut b = PdfBuilder::new();
    let form = b.add_form(
        "BT /F1 12 Tf 100 700 Td (INVOICE) Tj ET",
        dictionary! { "Resources" => font_resources() },
    );
    let doc = b.xobject("Fm1", form).content("/Fm1 Do").build();
    let page = doc.page(0).unwrap();
    let text = page
        .extract_text_layout(&TextOptions::pdfplumber_defaults())
        .unwrap();

    assert!(
        text.contains("INVOICE"),
        "layout output lost the form's text: {text:?}"
    );
}

#[test]
fn page_text_and_form_text_both_survive() {
    let mut b = PdfBuilder::new();
    let form = b.add_form(
        "BT /F1 12 Tf 100 600 Td (FORM) Tj ET",
        dictionary! { "Resources" => font_resources() },
    );
    let doc = b
        .xobject("Fm1", form)
        .font("F1", font())
        .content("BT /F1 12 Tf 100 700 Td (PAGE) Tj ET /Fm1 Do")
        .build();

    let text = text_of(&doc);
    assert!(text.contains("PAGE"), "lost the page's own text: {text:?}");
    assert!(text.contains("FORM"), "lost the form's text: {text:?}");
}

#[test]
fn form_matrix_offsets_the_glyphs() {
    // The same form, invoked once bare and once through a /Matrix that
    // translates 50 to the right. The glyph must move with it.
    let draw = "BT /F1 12 Tf 100 700 Td (X) Tj ET";
    let res = dictionary! { "Resources" => font_resources() };

    let mut plain = PdfBuilder::new();
    let f = plain.add_form(draw, res.clone());
    let plain = plain.xobject("Fm1", f).content("/Fm1 Do").build();

    let mut shifted = PdfBuilder::new();
    let mut with_matrix = res;
    with_matrix.set(
        "Matrix",
        vec![
            1.into(),
            0.into(),
            0.into(),
            1.into(),
            50.into(),
            Object::Integer(0),
        ],
    );
    let f = shifted.add_form(draw, with_matrix);
    let shifted = shifted.xobject("Fm1", f).content("/Fm1 Do").build();

    let a = plain.page(0).unwrap();
    let a = a.chars().unwrap();
    let b = shifted.page(0).unwrap();
    let b = b.chars().unwrap();

    assert_eq!(a.len(), 1, "expected one glyph, got {}", a.len());
    assert_eq!(b.len(), 1, "expected one glyph, got {}", b.len());
    assert!(
        (b[0].x0 - a[0].x0 - 50.0).abs() < 0.01,
        "/Matrix was not applied: {} vs {}",
        a[0].x0,
        b[0].x0
    );
}

#[test]
fn form_matrix_does_not_leak_into_later_page_content() {
    // `Do` is bracketed like q…Q, so the form's /Matrix must not survive it.
    let mut b = PdfBuilder::new();
    let mut entries = dictionary! { "Resources" => font_resources() };
    entries.set(
        "Matrix",
        vec![
            1.into(),
            0.into(),
            0.into(),
            1.into(),
            50.into(),
            Object::Integer(0),
        ],
    );
    let form = b.add_form("BT /F1 12 Tf 100 700 Td (F) Tj ET", entries);
    let doc = b
        .xobject("Fm1", form)
        .font("F1", font())
        .content("/Fm1 Do BT /F1 12 Tf 100 600 Td (P) Tj ET")
        .build();

    let page = doc.page(0).unwrap();
    let chars = page.chars().unwrap();
    let p = chars
        .iter()
        .find(|c| c.text == "P")
        .expect("page glyph missing");
    assert!(
        (p.x0 - 100.0).abs() < 0.01,
        "the form's /Matrix leaked past Do: x0 = {}",
        p.x0
    );
}

#[test]
fn form_uses_its_own_font_resources() {
    // The page and the form both define /F1, at different sizes. The glyph
    // drawn inside the form must be measured with the form's font.
    let mut b = PdfBuilder::new();
    let form = b.add_form(
        "BT /F1 20 Tf 100 700 Td (A) Tj ET",
        dictionary! {
            "Resources" => dictionary! {
                "Font" => dictionary! { "F1" => simple_font("WinAnsiEncoding", 32, &[1000; 96]) }
            }
        },
    );
    let doc = b
        .xobject("Fm1", form)
        .font("F1", font())
        .content("/Fm1 Do")
        .build();

    let page = doc.page(0).unwrap();
    let chars = page.chars().unwrap();
    assert_eq!(chars.len(), 1);
    // 1000/1000 * 20pt, not the page font's 500/1000.
    let width = chars[0].x1 - chars[0].x0;
    assert!(
        (width - 20.0).abs() < 0.01,
        "form font was ignored; glyph width = {width}"
    );
}

#[test]
fn form_without_resources_falls_back_to_the_page() {
    let mut b = PdfBuilder::new();
    let form = b.add_form("BT /F1 12 Tf 100 700 Td (OK) Tj ET", dictionary! {});
    let doc = b
        .xobject("Fm1", form)
        .font("F1", font())
        .content("/Fm1 Do")
        .build();

    assert_eq!(
        text_of(&doc),
        "OK",
        "a form with no /Resources must inherit the invoking stream's"
    );
}

#[test]
fn nested_forms_are_executed() {
    let mut b = PdfBuilder::new();
    let inner = b.add_form(
        "BT /F1 12 Tf 100 700 Td (INNER) Tj ET",
        dictionary! { "Resources" => font_resources() },
    );
    let outer = b.add_form(
        "/Fm2 Do",
        dictionary! {
            "Resources" => dictionary! { "XObject" => dictionary! { "Fm2" => Object::Reference(inner) } }
        },
    );
    let doc = b.xobject("Fm1", outer).content("/Fm1 Do").build();

    assert_eq!(
        text_of(&doc),
        "INNER",
        "a form nested in a form was skipped"
    );
}

#[test]
fn image_xobjects_are_ignored() {
    let mut b = PdfBuilder::new();
    let img = b.add_form(
        "BT /F1 12 Tf 100 700 Td (NOTTEXT) Tj ET",
        dictionary! { "Subtype" => "Image", "Resources" => font_resources() },
    );
    let doc = b.xobject("Im1", img).content("/Im1 Do").build();

    assert_eq!(
        text_of(&doc),
        "",
        "an /Image XObject's bytes are samples, not a content stream"
    );
}

#[test]
fn do_on_a_missing_xobject_is_survivable() {
    let doc = PdfBuilder::new()
        .font("F1", font())
        .content("/Nope Do BT /F1 12 Tf 100 700 Td (STILL) Tj ET")
        .build();

    assert_eq!(text_of(&doc), "STILL");
}

#[test]
fn self_referencing_form_terminates() {
    let text = with_timeout(10, || {
        // The form invokes itself. Nothing in the file prevents this.
        let mut b = PdfBuilder::new();
        let id = b.doc_next_id();
        let form = b.add_form_with_id(
            id,
            "BT /F1 12 Tf 100 700 Td (LOOP) Tj ET /Fm1 Do",
            dictionary! {
                "Resources" => dictionary! {
                    "Font" => dictionary! { "F1" => simple_font("WinAnsiEncoding", 32, &[500; 96]) },
                    "XObject" => dictionary! { "Fm1" => Object::Reference(id) }
                }
            },
        );
        let doc = b.xobject("Fm1", form).content("/Fm1 Do").build();
        text_of(&doc)
    })
    .expect("a self-referencing form must not hang the parser");

    assert!(
        text.starts_with("LOOP"),
        "the first pass through the form should still emit its text: {text:?}"
    );
}

#[test]
fn mutually_recursive_forms_terminate() {
    with_timeout(10, || {
        let mut b = PdfBuilder::new();
        let a_id = b.doc_next_id();
        let b_id = b.doc_next_id();
        b.add_form_with_id(
            a_id,
            "/FmB Do",
            dictionary! {
                "Resources" => dictionary! {
                    "XObject" => dictionary! { "FmB" => Object::Reference(b_id) }
                }
            },
        );
        b.add_form_with_id(
            b_id,
            "/FmA Do",
            dictionary! {
                "Resources" => dictionary! {
                    "XObject" => dictionary! { "FmA" => Object::Reference(a_id) }
                }
            },
        );
        let doc = b.xobject("FmA", a_id).content("/FmA Do").build();
        let _ = text_of(&doc);
    })
    .expect("two forms naming each other must not hang the parser");
}
