//! Counting outcomes, and keeping the few examples that make a count actionable.

use std::collections::HashMap;

/// A count per key, with the first example seen for each.
///
/// The example is what turns "1 204 unknown classes" into somewhere to look. First rather than
/// worst: there is no ordering on causes, and a stable one keeps two runs comparable.
#[derive(Debug, Default)]
pub(crate) struct Tally {
    counts: HashMap<String, (usize, String)>,
}

impl Tally {
    pub(crate) fn record(&mut self, key: &str, example: &str) {
        match self.counts.get_mut(key) {
            Some(entry) => entry.0 += 1,
            None => {
                self.counts.insert(key.to_owned(), (1, example.to_owned()));
            }
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// The `limit` most common keys, count first, and how many keys were left out.
    pub(crate) fn top(&self, limit: usize) -> (Vec<(usize, &str, &str)>, usize) {
        let mut rows = self
            .counts
            .iter()
            .map(|(key, (count, example))| (*count, key.as_str(), example.as_str()))
            .collect::<Vec<_>>();

        // Count descending, key ascending, so the same project always prints the same order.
        rows.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(right.1)));
        let remaining = rows.len().saturating_sub(limit);
        rows.truncate(limit);
        (rows, remaining)
    }
}
