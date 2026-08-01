//! Removal of double-struck glyphs.
//!
//! A generator whose font has no bold cut often fakes one by drawing the same
//! string twice, offset by a fraction of a point. Both copies are genuinely
//! in the file, so [`crate::page::Page::chars`] is right to report both — but
//! layout reconstruction then has two glyphs competing for one grid cell and
//! resolves it by interleaving them:
//!
//! ```text
//! SSAALLDDOO AANNTTEERRIIOORR   11..001122..002200,,4499
//! ```
//!
//! [`dedupe_chars`] drops the redundant copies. It is a port of pdfplumber's
//! `utils.text.dedupe_chars`, including its clustering rule, so output can be
//! compared against that implementation.

use crate::char::Char;

/// Drop glyphs that repeat another glyph's text, style and position.
///
/// Two glyphs are copies of one another when they agree on `text`,
/// `fontname`, `size` and `upright`, and sit within `tolerance` points of
/// each other on both axes. From each such cluster only one glyph survives:
/// the one earliest in `(doctop, x0)` order.
///
/// Clustering **chains**: a new cluster begins only where the gap to the
/// previous glyph exceeds `tolerance`, so three copies 0.6 apart form one
/// cluster at `tolerance = 1.0` even though the outermost two are 1.2 apart.
/// This matches the rule line grouping already uses.
///
/// Survivors come back in their original order, so stream order — which
/// [`crate::text::options::TextOptions::use_text_flow`] depends on — is
/// preserved.
///
/// A `tolerance` of `0.0` merges only glyphs at identical coordinates.
/// Glyphs with a non-finite `x0` or `doctop` are never merged with anything,
/// since no distance to them is meaningful; they are passed through
/// untouched rather than dropped.
///
/// # Example
///
/// ```
/// use pdfraw::text::dedupe::dedupe_chars;
/// use pdfraw::{Document, Result};
///
/// # fn run() -> Result<()> {
/// let doc = Document::open("statement.pdf")?;
/// let page = doc.page(0)?;
/// let chars = dedupe_chars(page.chars()?, 1.0);
/// # Ok(()) }
/// ```
pub fn dedupe_chars(chars: &[Char], tolerance: f32) -> Vec<Char> {
    if chars.len() < 2 {
        return chars.to_vec();
    }

    // Glyphs that agree on all four style fields are candidates for being
    // copies of each other; nothing else can be. Sorting brings each such
    // set together so the position clustering below only ever runs within
    // one of them.
    let mut order: Vec<usize> = (0..chars.len()).collect();
    order.sort_by(|&a, &b| style_key(&chars[a]).cmp(&style_key(&chars[b])));

    let mut keep = vec![true; chars.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && style_key(&chars[order[end]]) == style_key(&chars[order[start]])
        {
            end += 1;
        }
        dedupe_group(chars, &order[start..end], tolerance, &mut keep);
        start = end;
    }

    (0..chars.len())
        .filter(|&i| keep[i])
        .map(|i| chars[i].clone())
        .collect()
}

/// Everything about a glyph that makes it a different glyph rather than a
/// second impression of the same one.
///
/// `size` is compared through its bit pattern so the key can be `Ord`. Two
/// sizes that differ in the last bit are treated as distinct, which is what
/// the reference implementation does with float equality.
fn style_key(c: &Char) -> (bool, &str, &str, u32) {
    (
        c.upright,
        c.text.as_str(),
        c.fontname.as_str(),
        c.size.to_bits(),
    )
}

/// Collapse one style group, marking the losers in `keep`.
fn dedupe_group(chars: &[Char], group: &[usize], tolerance: f32, keep: &mut [bool]) {
    // Non-finite coordinates cannot be clustered — no distance to them
    // means anything — so they are left alone rather than folded into an
    // arbitrary neighbour.
    let (finite, _): (Vec<usize>, Vec<usize>) = group
        .iter()
        .partition(|&&i| chars[i].doctop.is_finite() && chars[i].x0.is_finite());

    for row in cluster_by(chars, &finite, tolerance, |c| c.doctop) {
        for column in cluster_by(chars, &row, tolerance, |c| c.x0) {
            // Ties break on position, not on stream order, so the survivor
            // is the same glyph however the file happened to order its two
            // impressions.
            let Some(&winner) = column.iter().min_by(|&&a, &&b| {
                let ka = (chars[a].doctop, chars[a].x0);
                let kb = (chars[b].doctop, chars[b].x0);
                ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
            }) else {
                continue;
            };
            for &i in &column {
                if i != winner {
                    keep[i] = false;
                }
            }
        }
    }
}

/// Chain-cluster indices by one coordinate: a new cluster starts where the
/// gap to the previous value exceeds `tolerance`.
fn cluster_by(
    chars: &[Char],
    indices: &[usize],
    tolerance: f32,
    coord: impl Fn(&Char) -> f32,
) -> Vec<Vec<usize>> {
    if indices.is_empty() {
        return Vec::new();
    }
    let mut sorted = indices.to_vec();
    sorted.sort_by(|&a, &b| {
        coord(&chars[a])
            .partial_cmp(&coord(&chars[b]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut out = vec![vec![sorted[0]]];
    let mut last = coord(&chars[sorted[0]]);
    for &i in &sorted[1..] {
        let v = coord(&chars[i]);
        if v - last <= tolerance {
            out.last_mut().expect("just pushed a cluster").push(i);
        } else {
            out.push(vec![i]);
        }
        last = v;
    }
    out
}
