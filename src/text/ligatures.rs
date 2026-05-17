//! Unicode ligature expansion.
//!
//! Some PDF fonts emit ligatures as a single glyph (e.g. `ﬁ` U+FB01). For
//! downstream consumers it is often useful to expand them to their multi-
//! character equivalents (`ﬁ → fi`). Expansion is opt-in via
//! [`crate::text::options::TextOptions::expand_ligatures`].

/// Replace any known ligature in `s` with its expanded form.
pub fn expand(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{FB00}' => out.push_str("ff"),
            '\u{FB01}' => out.push_str("fi"),
            '\u{FB02}' => out.push_str("fl"),
            '\u{FB03}' => out.push_str("ffi"),
            '\u{FB04}' => out.push_str("ffl"),
            '\u{FB05}' => out.push_str("ft"),
            '\u{FB06}' => out.push_str("st"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_unchanged() {
        assert_eq!(expand("hello world"), "hello world");
    }

    #[test]
    fn fi_ligature_expands() {
        assert_eq!(expand("\u{FB01}le"), "file");
    }

    #[test]
    fn ffi_ligature_expands() {
        assert_eq!(expand("e\u{FB03}cient"), "efficient");
    }
}
