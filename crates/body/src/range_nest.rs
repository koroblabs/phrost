//! Which of a list of ranges hold a position, found without reading the whole list.
//!
//! The ranges the tree hands out -- a declaration's reach, the span a guard covers -- nest or stay
//! apart, and every question asked of them is "which hold this offset", asked per position
//! resolved. Over a list of seven hundred test methods, or a function of three hundred guards,
//! reading the whole list per position was the largest cost of checking the file.
//!
//! When the ranges nest, the ones holding an offset are exactly the ancestors of the last range to
//! start at or before it: a range starting earlier that holds the offset either holds that one too
//! or ended before it started. So the answer is one binary search and a walk up a chain of
//! parents. A list whose ranges overlap without nesting is answered by reading it whole, as before.

use phrost_model::{TextRange, TextSize};

/// The nesting of a list of ranges, indexed by where each starts.
#[derive(Debug, Default)]
pub(crate) struct RangeNest {
    /// Positions in the original list, by start and then outermost first.
    order: Vec<u32>,
    /// For each entry of `order`, the nearest earlier entry whose range holds it.
    parents: Vec<Option<u32>>,
    starts: Vec<TextSize>,
    ranges: Vec<TextRange>,
    nests: bool,
    /// Whether every parent comes before its child in the original list, which makes an ancestor
    /// chain read backwards already in list order and saves sorting it.
    parents_first: bool,
}

/// The positions [`RangeNest::holding`] found, in list order, held on the stack for the depths code
/// is written at and spilled to the heap past them.
///
/// A position is resolved hundreds of thousands of times in a large file and each asks this, so the
/// two small vectors it used to build per ask were a tenth of the check.
#[derive(Debug)]
pub(crate) struct Holding {
    inline: [u32; INLINE],
    len: usize,
    spill: Vec<u32>,
    next: usize,
}

const INLINE: usize = 16;

impl Holding {
    fn push(&mut self, index: u32) {
        if self.len < INLINE {
            self.inline[self.len] = index;
        } else {
            if self.spill.is_empty() {
                self.spill.extend_from_slice(&self.inline);
            }
            self.spill.push(index);
        }
        self.len += 1;
    }

    fn as_mut_slice(&mut self) -> &mut [u32] {
        match self.spill.is_empty() {
            true => &mut self.inline[..self.len],
            false => &mut self.spill,
        }
    }
}

impl Iterator for Holding {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.next >= self.len {
            return None;
        }
        let found = match self.spill.is_empty() {
            true => self.inline[self.next],
            false => self.spill[self.next],
        };
        self.next += 1;
        Some(found)
    }
}

impl RangeNest {
    /// Indexes `ranges`, one per entry of the list the answers index into; `None` for an entry no
    /// offset can reach.
    pub(crate) fn of(ranges: impl Iterator<Item = Option<TextRange>>) -> Self {
        let mut entries: Vec<(u32, TextRange)> = ranges
            .enumerate()
            .filter_map(|(index, range)| {
                Some((u32::try_from(index).expect("fewer than 4G ranges"), range?))
            })
            .collect();
        // Outer before inner where two start together, so a parent is always met first.
        entries.sort_by_key(|(_, range)| (range.start(), std::cmp::Reverse(range.end())));
        let mut parents = Vec::with_capacity(entries.len());
        let mut open: Vec<usize> = Vec::new();
        let mut nests = true;
        for (position, (_, this)) in entries.iter().enumerate() {
            while open
                .last()
                .is_some_and(|&top| entries[top].1.end() < this.start())
            {
                open.pop();
            }
            if let Some(&top) = open.last()
                && entries[top].1.end() < this.end()
            {
                nests = false;
            }
            parents.push(
                open.last()
                    .map(|&top| u32::try_from(top).unwrap_or(u32::MAX)),
            );
            open.push(position);
        }
        let parents_first = parents.iter().enumerate().all(|(position, parent)| {
            parent.is_none_or(|parent| entries[parent as usize].0 < entries[position].0)
        });
        Self {
            parents_first,
            order: entries.iter().map(|(index, _)| *index).collect(),
            starts: entries.iter().map(|(_, range)| range.start()).collect(),
            ranges: entries.iter().map(|(_, range)| *range).collect(),
            parents,
            nests,
        }
    }

    /// The positions in the original list whose range holds `offset`, in the list's own order.
    ///
    /// `None` when the ranges do not nest, and the caller reads its list whole.
    pub(crate) fn holding(&self, offset: TextSize) -> Option<Holding> {
        if !self.nests {
            return None;
        }
        let mut found = Holding {
            inline: [0; INLINE],
            len: 0,
            spill: Vec::new(),
            next: 0,
        };
        let mut current = self
            .starts
            .partition_point(|&start| start <= offset)
            .checked_sub(1);
        while let Some(position) = current {
            if self.ranges[position].contains_inclusive(offset) {
                found.push(self.order[position]);
            }
            current = self.parents[position].map(|parent| parent as usize);
        }
        // Innermost first as walked; list order is the reverse wherever parents come first.
        match self.parents_first {
            true => found.as_mut_slice().reverse(),
            false => found.as_mut_slice().sort_unstable(),
        }
        Some(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(start: u32, end: u32) -> TextRange {
        TextRange::new(TextSize::from(start), TextSize::from(end))
    }

    /// The answer a whole-list read gives, which the index has to agree with exactly.
    fn by_reading(ranges: &[Option<TextRange>], offset: u32) -> Vec<u32> {
        ranges
            .iter()
            .enumerate()
            .filter(|(_, range)| range.is_some_and(|range| range.contains_inclusive(offset.into())))
            .map(|(index, _)| index as u32)
            .collect()
    }

    #[test]
    fn nesting_deeper_than_the_stack_buffer_answers_in_list_order() {
        // Forty ranges each inside the last, listed outermost first.
        let ranges: Vec<Option<TextRange>> = (0..40).map(|i| Some(range(i, 100 - i))).collect();
        let nest = RangeNest::of(ranges.iter().copied());
        for offset in [0, 20, 39, 50, 99] {
            let found: Vec<u32> = nest.holding(offset.into()).unwrap().collect();
            assert_eq!(found, by_reading(&ranges, offset), "at {offset}");
        }
    }

    #[test]
    fn a_parent_listed_after_its_children_is_still_answered_in_list_order() {
        // The shape of a file's declarations: the methods first, the file scope around them last.
        let ranges = vec![
            Some(range(10, 20)),
            Some(range(30, 40)),
            None,
            Some(range(32, 35)),
            Some(range(0, 100)),
        ];
        let nest = RangeNest::of(ranges.iter().copied());
        for offset in [5, 15, 33, 38, 99] {
            let found: Vec<u32> = nest.holding(offset.into()).unwrap().collect();
            assert_eq!(found, by_reading(&ranges, offset), "at {offset}");
        }
    }

    #[test]
    fn ranges_that_overlap_without_nesting_are_left_to_the_caller() {
        let nest = RangeNest::of([Some(range(0, 10)), Some(range(5, 15))].into_iter());
        assert!(nest.holding(7.into()).is_none());
    }
}
