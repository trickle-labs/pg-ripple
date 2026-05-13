//! Sorted trie iterators and in-memory edge data for Leapfrog Triejoin (v0.79.0).
//!
//! Contains `SortedIterator`, `leapfrog_intersect`, `EdgeData`, and `build_ranges`.

// ─── Sorted iterator ──────────────────────────────────────────────────────────

/// A sorted iterator over i64 values supporting O(log n) seek operations.
/// Implements the TrieIterator interface for the Leapfrog Triejoin algorithm.
pub struct SortedIterator {
    pub(super) values: Vec<i64>,
    pub(super) pos: usize,
}

impl SortedIterator {
    /// Create a new iterator from a list of values.  Values are sorted and
    /// deduplicated on construction.
    pub fn new(mut values: Vec<i64>) -> Self {
        values.sort_unstable();
        values.dedup();
        Self { values, pos: 0 }
    }

    /// Returns `true` when the iterator is exhausted.
    pub fn at_end(&self) -> bool {
        self.pos >= self.values.len()
    }

    /// The current key value.  Undefined when `at_end()`.
    pub fn key(&self) -> i64 {
        self.values[self.pos]
    }

    /// Advance to the next distinct value.
    pub fn next(&mut self) {
        if self.pos < self.values.len() {
            self.pos += 1;
        }
    }

    /// Advance so that `key() >= target`.  No-op when already satisfied.
    pub fn seek(&mut self, target: i64) {
        if self.pos >= self.values.len() {
            return;
        }
        if self.values[self.pos] >= target {
            return;
        }
        // Binary search in the remaining slice.
        let offset = self.values[self.pos..].partition_point(|&v| v < target);
        self.pos += offset;
    }

    /// Reset to the beginning of the iterator.
    // Q15-01: internal API field; kept for public API surface or future extension consumers.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.pos = 0;
    }
}

/// Intersect multiple sorted iterators using the Leapfrog algorithm.
///
/// Returns the sorted list of values that appear in **all** input iterators.
/// Achieves worst-case optimal O(N · log N) behaviour where N is the smallest
/// iterator's length, using binary-search seeks rather than linear scans.
pub fn leapfrog_intersect(iters: &mut [SortedIterator]) -> Vec<i64> {
    if iters.is_empty() {
        return vec![];
    }
    if iters.iter().any(|it| it.at_end()) {
        return vec![];
    }

    let mut result = Vec::new();
    // Start the leapfrog at the current maximum across all iterators.
    let mut x = iters.iter().map(|it| it.key()).max().unwrap_or(i64::MAX);

    'outer: loop {
        // Seek every iterator to x, tracking the new maximum.
        let mut new_max = x;
        for it in iters.iter_mut() {
            it.seek(x);
            if it.at_end() {
                break 'outer;
            }
            let k = it.key();
            if k > new_max {
                new_max = k;
            }
        }

        if new_max == x {
            // All iterators agree on x — emit the common value.
            result.push(x);
            // Advance all iterators past x.
            for it in iters.iter_mut() {
                it.next();
            }
            // Recalculate the starting point for the next round.
            let next_max = iters
                .iter()
                .filter_map(|it| if it.at_end() { None } else { Some(it.key()) })
                .max();
            match next_max {
                Some(v) => x = v,
                None => break 'outer,
            }
        } else {
            // Divergence — restart with the new maximum.
            x = new_max;
        }
    }

    result
}

// ─── Edge data ────────────────────────────────────────────────────────────────

/// In-memory edge data loaded from a single VP table.
///
/// Maintains two sorted indices — one by subject and one by object — to
/// support O(log n) range lookups for either column.
pub struct EdgeData {
    /// (s, o) pairs sorted by (s, o).
    pub(super) by_s: Vec<(i64, i64)>,
    /// (o, s) pairs sorted by (o, s).
    pub(super) by_o: Vec<(i64, i64)>,
    /// Index over `by_s`: for each unique s, the range [start..end).
    pub(super) s_ranges: Vec<(i64, usize, usize)>,
    /// Index over `by_o`: for each unique o, the range [start..end).
    pub(super) o_ranges: Vec<(i64, usize, usize)>,
}

/// Build a range index over a sorted (key, val) pair slice.
pub(super) fn build_ranges(pairs: &[(i64, i64)]) -> Vec<(i64, usize, usize)> {
    let mut ranges = Vec::new();
    let mut i = 0;
    while i < pairs.len() {
        let key = pairs[i].0;
        let start = i;
        while i < pairs.len() && pairs[i].0 == key {
            i += 1;
        }
        ranges.push((key, start, i));
    }
    ranges
}

impl EdgeData {
    /// Load edges from a VP table specified by its predicate ID.
    /// Returns `None` when the VP table does not exist or is empty.
    pub fn load_from_vp(pred_id: i64) -> Option<Self> {
        use pgrx::datum::DatumWithOid;
        use pgrx::prelude::*;

        // Check whether a dedicated VP table exists.
        let table_exists: bool = Spi::get_one_with_args::<bool>(
            "SELECT EXISTS(SELECT 1 FROM _pg_ripple.predicates WHERE id = $1 \
             AND table_oid IS NOT NULL)",
            &[DatumWithOid::from(pred_id)],
        )
        .ok()
        .flatten()
        .unwrap_or(false);

        let sql = if table_exists {
            format!(
                "SELECT s, o FROM _pg_ripple.vp_{pred_id} \
                 UNION ALL \
                 SELECT s, o FROM _pg_ripple.vp_{pred_id}_delta"
            )
        } else {
            // Fall back to vp_rare.
            format!("SELECT s, o FROM _pg_ripple.vp_rare WHERE p = {pred_id}")
        };

        let mut edges: Vec<(i64, i64)> = Vec::new();
        Spi::connect(|client| {
            if let Ok(rows) = client.select(&sql, None, &[]) {
                for row in rows {
                    if let (Ok(Some(s)), Ok(Some(o))) = (row.get::<i64>(1), row.get::<i64>(2)) {
                        edges.push((s, o));
                    }
                }
            }
        });

        if edges.is_empty() {
            return None;
        }

        edges.sort_unstable();
        edges.dedup();

        let by_s = edges.clone();
        let s_ranges = build_ranges(&by_s);

        let mut by_o: Vec<(i64, i64)> = edges.iter().map(|(s, o)| (*o, *s)).collect();
        by_o.sort_unstable();
        by_o.dedup();
        let o_ranges = build_ranges(&by_o);

        Some(Self {
            by_s,
            by_o,
            s_ranges,
            o_ranges,
        })
    }

    /// All unique subject values.
    pub fn all_s(&self) -> Vec<i64> {
        self.s_ranges.iter().map(|(k, _, _)| *k).collect()
    }

    /// All unique object values.
    pub fn all_o(&self) -> Vec<i64> {
        self.o_ranges.iter().map(|(k, _, _)| *k).collect()
    }

    /// All object values where subject = `s`.
    pub fn o_for_s(&self, s: i64) -> Vec<i64> {
        match self.s_ranges.binary_search_by_key(&s, |(k, _, _)| *k) {
            Ok(pos) => {
                let (_, start, end) = self.s_ranges[pos];
                self.by_s[start..end].iter().map(|(_, o)| *o).collect()
            }
            Err(_) => vec![],
        }
    }

    /// All subject values where object = `o`.
    pub fn s_for_o(&self, o: i64) -> Vec<i64> {
        match self.o_ranges.binary_search_by_key(&o, |(k, _, _)| *k) {
            Ok(pos) => {
                let (_, start, end) = self.o_ranges[pos];
                self.by_o[start..end].iter().map(|(_, s)| *s).collect()
            }
            Err(_) => vec![],
        }
    }

    /// Return `true` if the edge (s, o) exists.
    // Q15-01: internal API field; kept for public API surface or future extension consumers.
    #[allow(dead_code)]
    pub fn has_edge(&self, s: i64, o: i64) -> bool {
        self.by_s.binary_search(&(s, o)).is_ok()
    }

    /// Total number of edges.
    // Q15-01: internal API field; kept for public API surface or future extension consumers.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.by_s.len()
    }

    /// Return `true` if there are no edges.
    // Q15-01: internal API field; kept for public API surface or future extension consumers.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.by_s.is_empty()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(any(test, feature = "pg_test"))]
#[pgrx::pg_schema]
mod trie_tests {
    use super::*;

    #[test]
    fn test_sorted_iterator_seek() {
        let mut it = SortedIterator::new(vec![1, 3, 5, 7, 9]);
        assert_eq!(it.key(), 1);
        it.seek(4);
        assert_eq!(it.key(), 5);
        it.seek(5);
        assert_eq!(it.key(), 5);
        it.seek(10);
        assert!(it.at_end());
    }

    #[test]
    fn test_leapfrog_intersect_basic() {
        let mut iters = vec![
            SortedIterator::new(vec![1, 3, 5, 7]),
            SortedIterator::new(vec![2, 3, 6, 7]),
            SortedIterator::new(vec![3, 4, 7, 8]),
        ];
        let result = leapfrog_intersect(&mut iters);
        assert_eq!(result, vec![3, 7]);
    }

    #[test]
    fn test_leapfrog_intersect_empty() {
        let mut iters = vec![
            SortedIterator::new(vec![1, 2, 3]),
            SortedIterator::new(vec![4, 5, 6]),
        ];
        let result = leapfrog_intersect(&mut iters);
        assert!(result.is_empty());
    }

    #[test]
    fn test_leapfrog_intersect_single() {
        let mut iters = vec![SortedIterator::new(vec![1, 2, 3])];
        let result = leapfrog_intersect(&mut iters);
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_edge_data_lookup() {
        // Build edge data manually without SPI.
        let edges = vec![(1i64, 2i64), (1, 3), (2, 3), (3, 1)];
        let by_s = edges.clone();
        let s_ranges = build_ranges(&by_s);
        let mut by_o: Vec<(i64, i64)> = edges.iter().map(|(s, o)| (*o, *s)).collect();
        by_o.sort_unstable();
        let o_ranges = build_ranges(&by_o);
        let ed = EdgeData {
            by_s,
            by_o,
            s_ranges,
            o_ranges,
        };

        assert_eq!(ed.o_for_s(1), vec![2, 3]);
        assert_eq!(ed.o_for_s(2), vec![3]);
        assert_eq!(ed.s_for_o(1), vec![3]);
        assert!(ed.has_edge(2, 3));
        assert!(!ed.has_edge(2, 1));
    }
}
