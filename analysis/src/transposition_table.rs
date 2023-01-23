use std::fmt;

use tak::{Ply, ZobristHash};

use crate::evaluation::Evaluation;

const MAX_PROBE_DEPTH: isize = 5;

pub struct TranspositionTable<const N: usize> {
    len: usize,
    values: Vec<Option<Slot<N>>>,
}

impl<const N: usize> TranspositionTable<N> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            len: 0,
            values: vec![None; capacity],
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.values.len()
    }

    pub fn insert(&mut self, hash: ZobristHash, entry: TranspositionTableEntry<N>) -> bool {
        let start_index = self.index(hash);

        let mut current_index = start_index;
        let mut offset = 0;
        let mut target_index = start_index;
        let mut target_score = u16::MAX;

        fn score<const N: usize>(entry: &TranspositionTableEntry<N>) -> u16 {
            // Score by depth and then by ply count.
            // Prioritize lower over upper bounds, and exact over everything.
            ((entry.depth as u16) << 10) | ((entry.ply_count as u16) << 2) | (entry.bound as u16)
        }

        while offset < MAX_PROBE_DEPTH {
            if let Some(slot) = &self.values[current_index] {
                let slot_score = score(&slot.entry);

                if hash != slot.hash {
                    // Find the lowest score to replace.
                    if slot_score < target_score {
                        target_index = current_index;
                        target_score = slot_score;
                    }
                } else {
                    // If a slot contains the same hash, always attempt to replace this slot.
                    target_index = current_index;
                    target_score = slot_score;
                    break;
                }
            } else {
                // Always overwrite an empty slot.
                target_index = current_index;
                target_score = 0;
                break;
            }

            offset += 1;
            current_index = self.next_index(current_index);
        }

        // Only replace an old entry if we're actually improving things.
        let insert = target_score < score(&entry);
        if insert {
            // Only increase the len if we're not replacing an old entry.
            if self.values[target_index].is_none() {
                self.len += 1;
            }

            self.values[target_index] = Some(Slot { hash, entry });
        }
        insert
    }

    pub fn get(&self, hash: ZobristHash) -> Option<&TranspositionTableEntry<N>> {
        let mut current_index = self.index(hash);
        let mut offset = 0;

        while offset < MAX_PROBE_DEPTH {
            if let Some(slot) = &self.values[current_index] {
                if hash == slot.hash {
                    return Some(&slot.entry);
                }
            } else {
                return None;
            }

            offset += 1;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Slot<const N: usize> {
    hash: ZobristHash,
    entry: TranspositionTableEntry<N>,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bound {
    Upper = 0,
    Lower,
    Exact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranspositionTableEntry<const N: usize> {
    pub bound: Bound,
    pub evaluation: Evaluation,
    pub node_count: u32,
    pub depth: u8,
    pub ply_count: u8,
    pub ply: Ply<N>,
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

#[cfg(test)]
mod tests {
    use super::*;

    use tak::PieceType;

    fn test_entry<const N: usize>(value: u8) -> TranspositionTableEntry<N> {
        TranspositionTableEntry {
            bound: Bound::Exact,
            evaluation: 0.into(),
            node_count: 0,
            depth: value,
            ply_count: 0,
            ply: Ply::Place {
                x: 0,
                y: 0,
                piece_type: PieceType::Flatstone,
            },
        }
    }

    fn test_slot<const N: usize>(hash: ZobristHash, value: u8) -> Option<Slot<N>> {
        Some(Slot {
            hash,
            entry: test_entry(value),
        })
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
        let mut tt = TranspositionTable::<6>::with_capacity(10);
        assert!(tt.insert(3, test_entry(1)));
        assert!(tt.insert(13, test_entry(2)));
        assert_eq!(
            tt.values,
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
            tt.values,
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

        // When the probe depth is reached, insert should fail to insert a low score entry.
        assert!(!tt.insert(53, test_entry(1)));

        // But a higher one should replace the lowest within the probe range.
        assert!(tt.insert(63, test_entry(7)));
        assert_eq!(
            tt.values,
            vec![
                None,
                None,
                None,
                test_slot(3, 3),
                test_slot(63, 7),
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
            tt.values,
            vec![
                None,
                None,
                None,
                test_slot(3, 3),
                test_slot(63, 7),
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
            tt.values,
            vec![
                test_slot(19, 2),
                test_slot(29, 3),
                None,
                test_slot(3, 3),
                test_slot(63, 7),
                test_slot(23, 4),
                test_slot(33, 5),
                test_slot(43, 6),
                test_slot(4, 2),
                test_slot(9, 1),
            ]
        );

        // Get works when there's an exact match right away.
        assert_eq!(tt.get(3), Some(&test_entry(3)));

        // Get works when we have to probe for it.
        assert_eq!(tt.get(33), Some(&test_entry(5)));

        // Get returns none when there's an empty slot.
        assert_eq!(tt.get(2), None);

        // Get returns none when no match is found within the probe range.
        assert_eq!(tt.get(53), None);

        // Get works when probing has to wrap around.
        assert_eq!(tt.get(29), Some(&test_entry(3)));
    }
}
