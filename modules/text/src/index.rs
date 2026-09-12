//! Snapshot-local correspondences recorded only at decoded unit boundaries.

use alloc::vec::Vec;
use core::ops::Range;

pub const STEP: u64 = 1024;
const MARKS: usize = 4096;

#[derive(Clone, Copy, Default)]
pub struct Mark {
    pub unit: u64,
    pub byte: u64,
}

pub struct Index {
    marks: Vec<Mark>,
    stride: u64,
}

impl Default for Index {
    fn default() -> Self {
        Self { marks: Vec::new(), stride: STEP }
    }
}

impl Index {
    pub fn copy(&self) -> Option<Self> {
        let mut marks = Vec::new();
        marks.try_reserve_exact(self.marks.len()).ok()?;
        marks.extend_from_slice(&self.marks);
        Some(Self { marks, stride: self.stride })
    }

    /// Only the successful source splice supplies this correspondence.
    pub fn splice(
        &self,
        units: Range<u64>,
        bytes: Range<u64>,
        inserted_units: u64,
        inserted_bytes: u64,
    ) -> Option<Self> {
        let mut next = self.copy()?;
        let mut output = 0;
        for mark in self.marks.iter().copied().filter_map(|mark| {
            moved(mark, &units, &bytes, inserted_units, inserted_bytes)
        }) {
            next.marks[output] = mark;
            output += 1;
        }
        next.marks.truncate(output);
        next.marks.dedup_by_key(|mark| mark.unit);
        Some(next)
    }
    pub fn before(&self, unit: u64) -> Mark {
        let end = self.marks.partition_point(|mark| mark.unit <= unit);
        end.checked_sub(1).map_or_else(Mark::default, |i| self.marks[i])
    }

    pub fn record(&mut self, mark: Mark) {
        if mark.unit == 0 || !mark.unit.is_multiple_of(self.stride) {
            return;
        }
        if self.marks.binary_search_by_key(&mark.unit, |m| m.unit).is_ok() {
            return;
        }
        if self.marks.len() == MARKS && !self.thin() {
            return;
        }
        if !mark.unit.is_multiple_of(self.stride) {
            return;
        }
        let additional = if self.marks.len() == self.marks.capacity() {
            self.marks.capacity().saturating_mul(2).clamp(4, MARKS)
                - self.marks.len()
        } else {
            0
        };
        if self.marks.try_reserve_exact(additional).is_ok() {
            let slot = self.marks.partition_point(|old| old.unit < mark.unit);
            self.marks.insert(slot, mark);
        }
    }

    fn thin(&mut self) -> bool {
        let Some(stride) = self.stride.checked_mul(2) else {
            return false;
        };
        self.stride = stride;
        let mut keep = true;
        self.marks.retain(|_| {
            keep = !keep;
            keep
        });
        true
    }
}

fn moved(
    mark: Mark,
    units: &Range<u64>,
    bytes: &Range<u64>,
    inserted_units: u64,
    inserted_bytes: u64,
) -> Option<Mark> {
    if mark.unit <= units.start {
        return Some(mark);
    }
    if mark.unit < units.end {
        return None;
    }
    Some(Mark {
        unit: mark
            .unit
            .checked_sub(units.end - units.start)?
            .checked_add(inserted_units)?,
        byte: mark
            .byte
            .checked_sub(bytes.end - bytes.start)?
            .checked_add(inserted_bytes)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{Index, MARKS, Mark, STEP};

    #[test]
    fn thinning_keeps_exact_predecessors_and_ignores_duplicates() {
        let mut index = Index::default();
        for i in 1..=MARKS as u64 {
            index.record(Mark { unit: i * STEP, byte: i * STEP * 3 });
        }
        index.record(Mark { unit: STEP, byte: STEP * 3 });
        assert_eq!(index.stride, STEP);
        index.record(Mark {
            unit: (MARKS as u64 + 2) * STEP,
            byte: (MARKS as u64 + 2) * STEP * 3,
        });
        assert!(index.marks.len() <= MARKS);
        assert_eq!(index.before(STEP).unit, 0);
        let found = index.before(7 * STEP);
        assert_eq!((found.unit, found.byte), (6 * STEP, 18 * STEP));
        let tail = index.before(u64::MAX);
        assert_eq!(tail.byte, tail.unit * 3);
    }

    #[test]
    fn known_splice_preserves_prefix_and_moves_only_surviving_suffix_points() {
        let mut index = Index::default();
        for i in 1..=8 {
            index.record(Mark { unit: i * STEP, byte: i * STEP * 3 });
        }
        let next = index
            .splice(2 * STEP..4 * STEP, 6 * STEP..12 * STEP, 1, 1)
            .unwrap();
        assert_eq!(next.before(2 * STEP).byte, 6 * STEP);
        assert_eq!(next.before(2 * STEP + 1).byte, 6 * STEP + 1);
        assert_eq!(next.before(3 * STEP + 1).byte, 9 * STEP + 1);
        assert_eq!(index.before(4 * STEP).byte, 12 * STEP);
    }

    #[test]
    fn coarsening_keeps_valid_points_after_an_insertion_shifts_the_grid() {
        let mut index = Index::default();
        for i in 1..=MARKS as u64 {
            index.record(Mark { unit: i * STEP, byte: i * STEP * 3 });
        }
        let mut next = index.splice(0..0, 0..0, 1, 1).unwrap();
        let unit = (MARKS as u64 + 2) * STEP;
        next.record(Mark { unit, byte: unit * 3 - 2 });
        let found = next.before(7 * STEP + 1);
        assert_eq!((found.unit, found.byte), (6 * STEP + 1, 18 * STEP + 1));
        assert!(next.marks.len() <= MARKS);
    }

    #[test]
    fn copied_indexes_keep_the_allocation_bound_when_growing() {
        let mut original = Index::default();
        for i in 1..=137 {
            original.record(Mark { unit: i * STEP, byte: i * STEP });
        }
        let mut copied = original.copy().unwrap();
        for i in 138..=MARKS as u64 {
            copied.record(Mark { unit: i * STEP, byte: i * STEP });
        }
        assert!(copied.marks.capacity() <= MARKS);
        assert_eq!(copied.before(u64::MAX).unit, MARKS as u64 * STEP);
    }
}
