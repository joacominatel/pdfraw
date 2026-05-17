//! Greedy 1D clustering.
//!
//! Given a slice of items and a function returning a numeric key, group
//! items whose successive keys differ by no more than `tolerance`. Used to
//! cluster chars into lines (by `top`) and into columns (by `x0`).

/// Cluster `items` into contiguous groups by key.
///
/// Algorithm: sort items by `key`, then walk sorted order grouping while
/// `key(curr) <= key(last_in_group) + tolerance`.
///
/// Returns the groups in key-ascending order; each group preserves the
/// relative order of its members.
pub fn cluster_objects<T, K>(items: &[T], key: K, tolerance: f32) -> Vec<Vec<&T>>
where
    K: Fn(&T) -> f32,
{
    if items.is_empty() {
        return Vec::new();
    }

    let mut indexed: Vec<(usize, f32)> =
        items.iter().enumerate().map(|(i, x)| (i, key(x))).collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut groups: Vec<Vec<&T>> = Vec::new();
    let mut last_key = indexed[0].1;
    let mut current: Vec<&T> = vec![&items[indexed[0].0]];

    for &(idx, k) in &indexed[1..] {
        if k <= last_key + tolerance {
            current.push(&items[idx]);
        } else {
            groups.push(std::mem::take(&mut current));
            current.push(&items[idx]);
        }
        last_key = k;
    }
    groups.push(current);
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_no_groups() {
        let v: Vec<f32> = vec![];
        let g = cluster_objects(&v, |x| *x, 1.0);
        assert!(g.is_empty());
    }

    #[test]
    fn all_within_tolerance_yields_one_group() {
        let v = vec![1.0_f32, 1.5, 2.0, 2.4];
        let g = cluster_objects(&v, |x| *x, 1.0);
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn gap_larger_than_tolerance_splits_group() {
        let v = vec![1.0_f32, 1.5, 5.0, 5.4];
        let g = cluster_objects(&v, |x| *x, 1.0);
        assert_eq!(g.len(), 2);
    }

    #[test]
    fn unsorted_input_is_handled() {
        let v = vec![5.0_f32, 1.0, 5.5, 1.5];
        let g = cluster_objects(&v, |x| *x, 1.0);
        assert_eq!(g.len(), 2);
    }

    #[test]
    fn chain_within_tolerance_stays_one_group() {
        // each item is within 1.0 of the previous, even though first and
        // last differ by 3.0 — they should still cluster together
        let v = vec![0.0_f32, 0.9, 1.7, 2.5, 3.0];
        let g = cluster_objects(&v, |x| *x, 1.0);
        assert_eq!(g.len(), 1);
    }
}
