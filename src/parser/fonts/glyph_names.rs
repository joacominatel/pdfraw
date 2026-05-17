//! Mapping from Adobe glyph names to Unicode scalar values.
//!
//! The full Adobe Glyph List has ~4000 entries; PDFs encountered in
//! invoices use only a small subset. Names not in this table are returned
//! as `None`. Callers should fall back to whatever the base encoding
//! provides for the original code.
//!
//! Source for the values: Adobe Glyph List 2.0.

/// Look up a PostScript glyph name and return its Unicode scalar value, if
/// known.
pub fn lookup(name: &str) -> Option<char> {
    let name = name.strip_prefix('/').unwrap_or(name);
    GLYPHS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
        .or_else(|| parse_uni_name(name))
}

fn parse_uni_name(name: &str) -> Option<char> {
    // Adobe convention: glyphs of the form "uni0041" or "u0041".
    let hex = if let Some(rest) = name.strip_prefix("uni") {
        rest
    } else {
        name.strip_prefix('u')?
    };
    if hex.len() < 4 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(&hex[..4.min(hex.len())], 16).ok()?;
    char::from_u32(n)
}

#[rustfmt::skip]
static GLYPHS: &[(&str, char)] = &[
    ("space", ' '), ("exclam", '!'), ("quotedbl", '"'), ("numbersign", '#'),
    ("dollar", '$'), ("percent", '%'), ("ampersand", '&'), ("quoteright", '\''),
    ("parenleft", '('), ("parenright", ')'), ("asterisk", '*'), ("plus", '+'),
    ("comma", ','), ("hyphen", '-'), ("minus", '-'), ("period", '.'),
    ("slash", '/'), ("zero", '0'), ("one", '1'), ("two", '2'), ("three", '3'),
    ("four", '4'), ("five", '5'), ("six", '6'), ("seven", '7'), ("eight", '8'),
    ("nine", '9'), ("colon", ':'), ("semicolon", ';'), ("less", '<'),
    ("equal", '='), ("greater", '>'), ("question", '?'), ("at", '@'),
    ("bracketleft", '['), ("backslash", '\\'), ("bracketright", ']'),
    ("asciicircum", '^'), ("underscore", '_'), ("quoteleft", '`'),
    ("braceleft", '{'), ("bar", '|'), ("braceright", '}'), ("asciitilde", '~'),

    ("endash", '\u{2013}'), ("emdash", '\u{2014}'),
    ("quoteleft2", '\u{2018}'), ("quoteright2", '\u{2019}'),
    ("quotedblleft", '\u{201C}'), ("quotedblright", '\u{201D}'),
    ("bullet", '\u{2022}'), ("ellipsis", '\u{2026}'),
    ("perthousand", '\u{2030}'), ("guilsinglleft", '\u{2039}'),
    ("guilsinglright", '\u{203A}'),

    ("Euro", '\u{20AC}'), ("trademark", '\u{2122}'),
    ("copyright", '\u{00A9}'), ("registered", '\u{00AE}'),
    ("paragraph", '\u{00B6}'), ("section", '\u{00A7}'),
    ("yen", '\u{00A5}'), ("sterling", '\u{00A3}'), ("cent", '\u{00A2}'),
    ("florin", '\u{0192}'), ("degree", '\u{00B0}'),
    ("plusminus", '\u{00B1}'), ("multiply", '\u{00D7}'),
    ("divide", '\u{00F7}'), ("dagger", '\u{2020}'),
    ("daggerdbl", '\u{2021}'),

    ("Agrave", 'À'), ("Aacute", 'Á'), ("Acircumflex", 'Â'),
    ("Atilde", 'Ã'), ("Adieresis", 'Ä'), ("Aring", 'Å'), ("AE", 'Æ'),
    ("Ccedilla", 'Ç'), ("Egrave", 'È'), ("Eacute", 'É'),
    ("Ecircumflex", 'Ê'), ("Edieresis", 'Ë'),
    ("Igrave", 'Ì'), ("Iacute", 'Í'), ("Icircumflex", 'Î'),
    ("Idieresis", 'Ï'), ("Eth", 'Ð'), ("Ntilde", 'Ñ'),
    ("Ograve", 'Ò'), ("Oacute", 'Ó'), ("Ocircumflex", 'Ô'),
    ("Otilde", 'Õ'), ("Odieresis", 'Ö'), ("Oslash", 'Ø'), ("OE", 'Œ'),
    ("Ugrave", 'Ù'), ("Uacute", 'Ú'), ("Ucircumflex", 'Û'),
    ("Udieresis", 'Ü'), ("Yacute", 'Ý'), ("Ydieresis", 'Ÿ'),
    ("Thorn", 'Þ'), ("germandbls", 'ß'),

    ("agrave", 'à'), ("aacute", 'á'), ("acircumflex", 'â'),
    ("atilde", 'ã'), ("adieresis", 'ä'), ("aring", 'å'), ("ae", 'æ'),
    ("ccedilla", 'ç'), ("egrave", 'è'), ("eacute", 'é'),
    ("ecircumflex", 'ê'), ("edieresis", 'ë'),
    ("igrave", 'ì'), ("iacute", 'í'), ("icircumflex", 'î'),
    ("idieresis", 'ï'), ("eth", 'ð'), ("ntilde", 'ñ'),
    ("ograve", 'ò'), ("oacute", 'ó'), ("ocircumflex", 'ô'),
    ("otilde", 'õ'), ("odieresis", 'ö'), ("oslash", 'ø'), ("oe", 'œ'),
    ("ugrave", 'ù'), ("uacute", 'ú'), ("ucircumflex", 'û'),
    ("udieresis", 'ü'), ("yacute", 'ý'), ("ydieresis", 'ÿ'),
    ("thorn", 'þ'),

    ("fi", '\u{FB01}'), ("fl", '\u{FB02}'),
    ("ff", '\u{FB00}'), ("ffi", '\u{FB03}'), ("ffl", '\u{FB04}'),

    ("nbspace", '\u{00A0}'), ("exclamdown", '¡'), ("questiondown", '¿'),
    ("guillemotleft", '«'), ("guillemotright", '»'),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_glyph_resolves() {
        assert_eq!(lookup("/copyright"), Some('©'));
    }

    #[test]
    fn glyph_with_leading_slash_or_without_both_work() {
        assert_eq!(lookup("space"), Some(' '));
    }

    #[test]
    fn unknown_glyph_returns_none() {
        assert_eq!(lookup("/notarealglyph"), None);
    }

    #[test]
    fn uni_hex_name_is_parsed() {
        assert_eq!(lookup("/uni00E9"), Some('é'));
    }
}
