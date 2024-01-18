use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::{fmt, mem};

use tak::{Ply, ZobristHash};

use crate::evaluation::Evaluation;
use crate::util::PackedPly;

const MAX_PROBE_DEPTH: isize = 5;

pub struct TranspositionTable<const N: usize> {
    len: AtomicUsize,
    values: Vec<Slot<N>>,
}

impl<const N: usize> TranspositionTable<N> {
    pub fn with_capacity(capacity: usize) -> Self {
        let mut values = Vec::with_capacity(capacity);
        values.resize_with(capacity, Default::default);

        Self {
            len: AtomicUsize::new(0),
            values,
        }
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.values.len()
    }

    pub fn insert(&self, hash: ZobristHash, entry: TranspositionTableEntry<N>) -> bool {
        let start_index = self.index(hash);

        let mut current_index = start_index;
        let mut target_index = start_index;
        let mut target_score = u32::MAX;

        fn score<const N: usize>(entry: TranspositionTableEntry<N>) -> u32 {
            // Score by total ply depth, bound, and then individual search depth.
            ((entry.depth() as u32 + entry.ply_count() as u32) << 16)
                | ((entry.bound() as u32) << 8)
                | (entry.depth() as u32)
        }

        let entry_score = score(entry);

        for _ in 0..MAX_PROBE_DEPTH {
            if let Some(slot) = self.values[current_index].load() {
                let slot_score = score(slot.entry);

                if hash != slot.hash {
                    // Find the lowest score to replace.
                    if slot_score < target_score {
                        target_index = current_index;
                        target_score = slot_score;
                    }
                } else if entry_score >= slot_score {
                    self.values[current_index].store(hash, entry);
                    return true;
                } else {
                    return false;
                }
            } else {
                // Always overwrite an empty slot.
                target_index = current_index;
                self.len.fetch_add(1, Ordering::AcqRel);
                break;
            }

            current_index = self.next_index(current_index);
        }

        self.values[target_index].store(hash, entry);

        true
    }

    pub fn get(&self, hash: ZobristHash) -> Option<TranspositionTableEntry<N>> {
        let mut current_index = self.index(hash);

        for _ in 0..MAX_PROBE_DEPTH {
            if let Some(slot) = self.values[current_index].load() {
                if hash == slot.hash {
                    return Some(slot.entry);
                }
            } else {
                return None;
            }

            current_index = self.next_index(current_index);
        }

        None
    }
}

impl<const N: usize> fmt::Debug for TranspositionTable<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TranspositionTable")
            .field("occupancy", &self.len)
            .field("capacity", &self.capacity())
            .finish()
    }
}

impl<const N: usize> TranspositionTable<N> {
    fn index(&self, hash: ZobristHash) -> usize {
        hash as usize % self.capacity()
    }

    fn next_index(&self, index: usize) -> usize {
        let next = index + 1;
        if next < self.capacity() {
            next
        } else {
            next - self.capacity()
        }
    }
}

#[derive(Debug, Default)]
struct Slot<const N: usize> {
    key: AtomicU64,
    data: AtomicU64,
}

impl<const N: usize> Slot<N> {
    fn load(&self) -> Option<LoadedSlot<N>> {
        let key = self.key.load(Ordering::Acquire);

        if key == 0 {
            None
        } else {
            let data = self.data.load(Ordering::Acquire);
            Some(LoadedSlot {
                hash: key ^ data,
                entry: TranspositionTableEntry::from_bits(data),
            })
        }
    }

    fn store(&self, hash: ZobristHash, entry: TranspositionTableEntry<N>) {
        let data = entry.to_bits();
        let key = hash ^ data;

        // Store the data before the key.  Since we read the key first upon load,
        // this will ensure that the data will never be an invalid entry while the
        // key is non-zero.
        self.data.store(data, Ordering::Release);
        self.key.store(key, Ordering::Release);
    }
}

#[derive(Debug, PartialEq)]
struct LoadedSlot<const N: usize> {
    hash: ZobristHash,
    entry: TranspositionTableEntry<N>,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bound {
    Lower = 0,
    Upper,
    Exact,
}

impl TryFrom<u8> for Bound {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Bound::Lower),
            1 => Ok(Bound::Upper),
            2 => Ok(Bound::Exact),
            _ => Err("invalid bound value"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranspositionTableEntry<const N: usize> {
    ply: PackedPly,
    evaluation: Evaluation,
    info: EntryInfo,
}

impl<const N: usize> TranspositionTableEntry<N> {
    pub fn new(
        ply: Ply<N>,
        evaluation: Evaluation,
        bound: Bound,
        depth: usize,
        ply_count: u16,
    ) -> Self {
        Self {
            ply: ply.into(),
            evaluation,
            info: EntryInfo::new(bound, depth, ply_count),
        }
    }

    pub fn ply(&self) -> Ply<N> {
        self.ply.try_into().expect("could not unpack ply")
    }

    pub fn evaluation(&self) -> Evaluation {
        self.evaluation
    }

    pub fn bound(&self) -> Bound {
        self.info.bound()
    }

    pub fn depth(&self) -> usize {
        self.info.depth()
    }

    pub fn ply_count(&self) -> u16 {
        self.info.ply_count()
    }
}

impl<const N: usize> TranspositionTableEntry<N> {
    fn to_bits(self) -> u64 {
        debug_assert_eq!(mem::size_of::<Self>(), mem::size_of::<u64>(),);

        unsafe { mem::transmute(self) }
    }

    fn from_bits(value: u64) -> Self {
        debug_assert_eq!(mem::size_of::<Self>(), mem::size_of::<u64>(),);

        unsafe { mem::transmute(value) }
    }
}

/// Bit-packed bound, depth, and ply_count information. Representation:
/// ```text
///     Bound  ┊  Depth   ┊   Ply count
///       ├─┐ ┌─────┴─┐ ┌───────┴───────┐
/// MSB - b b d d d d d p p p p p p p p p - LSB
/// ```
/// This does impose limits on the possible depth and ply count.
/// The maximum depth is 32, and the maximum ply count is 511.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EntryInfo(u16);

const ENTRY_MAX_DEPTH: usize = 32;
const ENTRY_MAX_PLY_COUNT: u16 = 511;

impl EntryInfo {
    fn new(bound: Bound, depth: usize, ply_count: u16) -> Self {
        assert!(depth > 0, "transposition table entry depth cannot be 0");
        assert!(
            depth <= ENTRY_MAX_DEPTH,
            "transposition table entry depth cannot be greater than {ENTRY_MAX_DEPTH}"
        );
        assert!(
            ply_count <= ENTRY_MAX_PLY_COUNT,
            "transposition table entry ply count cannot be greater than {ENTRY_MAX_PLY_COUNT}"
        );

        Self(((bound as u16) << 14) | ((depth as u16 - 1) << 9) | ply_count)
    }

    fn bound(self) -> Bound {
        ((self.0 >> 14) as u8)
            .try_into()
            .expect("invalid packed bound")
    }

    fn depth(self) -> usize {
        ((self.0 & 0x3E00) >> 9) as usize + 1
    }

    fn ply_count(self) -> u16 {
        self.0 & 0x01FF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tak::PieceType;

    fn test_entry<const N: usize>(value: usize) -> TranspositionTableEntry<N> {
        TranspositionTableEntry::new(
            Ply::Place {
                x: 0,
                y: 0,
                piece_type: PieceType::Flatstone,
            },
            0.0.into(),
            Bound::Exact,
            value,
            0,
        )
    }

    fn test_slot<const N: usize>(hash: ZobristHash, value: usize) -> Option<LoadedSlot<N>> {
        Some(LoadedSlot {
            hash,
            entry: test_entry(value),
        })
    }

    fn load<const N: usize>(values: &[Slot<N>]) -> Vec<Option<LoadedSlot<N>>> {
        values.iter().map(|slot| slot.load()).collect()
    }

    #[test]
    fn slot_print_size() {
        for n in 3..=8 {
            let (size, alignment) = match n {
                3 => (
                    std::mem::size_of::<Slot<3>>(),
                    std::mem::align_of::<Slot<3>>(),
                ),
                4 => (
                    std::mem::size_of::<Slot<4>>(),
                    std::mem::align_of::<Slot<4>>(),
                ),
                5 => (
                    std::mem::size_of::<Slot<5>>(),
                    std::mem::align_of::<Slot<5>>(),
                ),
                6 => (
                    std::mem::size_of::<Slot<6>>(),
                    std::mem::align_of::<Slot<6>>(),
                ),
                7 => (
                    std::mem::size_of::<Slot<7>>(),
                    std::mem::align_of::<Slot<7>>(),
                ),
                8 => (
                    std::mem::size_of::<Slot<8>>(),
                    std::mem::align_of::<Slot<8>>(),
                ),
                _ => unreachable!(),
            };

            println!("Slot<{n}>: {size} bytes, {alignment} byte alignment");
        }
    }

    #[test]
    fn entry_print_size() {
        for n in 3..=8 {
            let (size, alignment) = match n {
                3 => (
                    std::mem::size_of::<TranspositionTableEntry<3>>(),
                    std::mem::align_of::<TranspositionTableEntry<3>>(),
                ),
                4 => (
                    std::mem::size_of::<TranspositionTableEntry<4>>(),
                    std::mem::align_of::<TranspositionTableEntry<4>>(),
                ),
                5 => (
                    std::mem::size_of::<TranspositionTableEntry<5>>(),
                    std::mem::align_of::<TranspositionTableEntry<5>>(),
                ),
                6 => (
                    std::mem::size_of::<TranspositionTableEntry<6>>(),
                    std::mem::align_of::<TranspositionTableEntry<6>>(),
                ),
                7 => (
                    std::mem::size_of::<TranspositionTableEntry<7>>(),
                    std::mem::align_of::<TranspositionTableEntry<7>>(),
                ),
                8 => (
                    std::mem::size_of::<TranspositionTableEntry<8>>(),
                    std::mem::align_of::<TranspositionTableEntry<8>>(),
                ),
                _ => unreachable!(),
            };

            println!("TranspositionTableEntry<{n}>: {size} bytes, {alignment} byte alignment");
        }
    }

    #[test]
    fn insert_and_get() {
        // Probe to find an empty slot.
        let tt = TranspositionTable::<6>::with_capacity(10);
        assert!(tt.insert(3, test_entry(1)));
        assert!(tt.insert(13, test_entry(2)));
        assert_eq!(
            load(&tt.values),
            vec![
                None,
                None,
                None,
                test_slot(3, 1),
                test_slot(13, 2),
                None,
                None,
                None,
                None,
                None
            ]
        );

        // Overwrite entries with the same hash but a higher score.
        assert!(tt.insert(3, test_entry(3)));
        assert_eq!(
            load(&tt.values),
            vec![
                None,
                None,
                None,
                test_slot(3, 3),
                test_slot(13, 2),
                None,
                None,
                None,
                None,
                None
            ]
        );

        assert!(tt.insert(23, test_entry(4)));
        assert!(tt.insert(33, test_entry(5)));
        assert!(tt.insert(43, test_entry(6)));

        // When the probe depth is reached, insert should replace the lowest score entry.
        assert!(tt.insert(53, test_entry(1)));
        assert_eq!(
            load(&tt.values),
            vec![
                None,
                None,
                None,
                test_slot(3, 3),
                test_slot(53, 1),
                test_slot(23, 4),
                test_slot(33, 5),
                test_slot(43, 6),
                None,
                None
            ]
        );

        // Adjacent indices probe further.
        assert!(tt.insert(4, test_entry(2)));
        assert_eq!(
            load(&tt.values),
            vec![
                None,
                None,
                None,
                test_slot(3, 3),
                test_slot(53, 1),
                test_slot(23, 4),
                test_slot(33, 5),
                test_slot(43, 6),
                test_slot(4, 2),
                None
            ]
        );

        // Probed indices wrap around.
        assert!(tt.insert(9, test_entry(1)));
        assert!(tt.insert(19, test_entry(2)));
        assert!(tt.insert(29, test_entry(3)));
        assert_eq!(
            load(&tt.values),
            vec![
                test_slot(19, 2),
                test_slot(29, 3),
                None,
                test_slot(3, 3),
                test_slot(53, 1),
                test_slot(23, 4),
                test_slot(33, 5),
                test_slot(43, 6),
                test_slot(4, 2),
                test_slot(9, 1),
            ]
        );

        // Get works when there's an exact match right away.
        assert_eq!(tt.get(3), Some(test_entry(3)));

        // Get works when we have to probe for it.
        assert_eq!(tt.get(33), Some(test_entry(5)));

        // Get returns none when there's an empty slot.
        assert_eq!(tt.get(2), None);

        // Get returns none when no match is found within the probe range.
        assert_eq!(tt.get(63), None);

        // Get works when probing has to wrap around.
        assert_eq!(tt.get(29), Some(test_entry(3)));
    }

    #[test]
    fn entry_info() {
        let entry_info = EntryInfo::new(Bound::Exact, 32, 511);
        assert_eq!(entry_info.0, 0xBFFF);
        assert_eq!(entry_info.bound(), Bound::Exact);
        assert_eq!(entry_info.depth(), 32);
        assert_eq!(entry_info.ply_count(), 511);
    }
}
