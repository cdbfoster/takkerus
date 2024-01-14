use std::fmt;

use tak::{Drops, Ply, PlyError, ZobristHash};

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
        let mut target_score = u32::MAX;

        fn score<const N: usize>(entry: &TranspositionTableEntry<N>) -> u32 {
            // Score by total ply depth, bound, and then individual search depth.
            ((entry.depth() as u32 + entry.ply_count() as u32) << 16)
                | ((entry.bound() as u32) << 8)
                | (entry.depth() as u32)
        }

        let entry_score = score(&entry);

        while offset < MAX_PROBE_DEPTH {
            if let Some(slot) = &self.values[current_index] {
                let slot_score = score(&slot.entry);

                if hash != slot.hash {
                    // Find the lowest score to replace.
                    if slot_score < target_score {
                        target_index = current_index;
                        target_score = slot_score;
                    }
                } else if entry_score >= slot_score {
                    self.values[current_index] = Some(Slot { hash, entry });
                    return true;
                } else {
                    return false;
                }
            } else {
                // Always overwrite an empty slot.
                target_index = current_index;
                self.len += 1;
                break;
            }

            offset += 1;
            current_index = self.next_index(current_index);
        }

        self.values[target_index] = Some(Slot { hash, entry });

        true
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

/// A bit-packed ply. Representation:
/// ```text
/// Place:
///               Magic  ┊   Type ┊ X coord ┊ Y coord
///           ┌─────┴───────┐   ├─┐ ┌──┴┐ ┌───┤
///     MSB - 1 1 0 0 0 0 0 0 , t t x x x y y y - LSB
///
/// Spread:
///   Direction ┊ X coord ┊ Y coord ┊ Drop pattern
///           ├─┐ ┌──┴┐ ┌───┤   ┌──────────┴──┐
///     MSB - d d x x x y y y , d … … … … … … d - LSB
/// ```
/// These patterns are distinguishable because the "magic" value
/// cannot be interpreted as a valid spread; it would represent a
/// spread West from (0, 0), which is impossible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PackedPly(u8, u8);

impl<const N: usize> From<Ply<N>> for PackedPly {
    fn from(ply: Ply<N>) -> Self {
        match ply {
            Ply::Place { x, y, piece_type } => {
                PackedPly(0b11000000, ((piece_type as u8 & 0xE0) << 1) | (x << 3) | y)
            }
            Ply::Spread {
                x,
                y,
                direction,
                drops,
            } => PackedPly(((direction as u8) << 6) | (x << 3) | y, drops.into()),
        }
    }
}

impl<const N: usize> TryFrom<PackedPly> for Ply<N> {
    type Error = PlyError;

    fn try_from(packed: PackedPly) -> Result<Self, Self::Error> {
        let ply = if packed.0 == 0b11000000 {
            Ply::Place {
                x: (packed.1 >> 3) & 0x07,
                y: packed.1 & 0x07,
                piece_type: (0x01 << ((packed.1 >> 6) + 4))
                    .try_into()
                    .expect("invalid packed piece type"),
            }
        } else {
            Ply::Spread {
                x: (packed.0 >> 3) & 0x07,
                y: packed.0 & 0x07,
                direction: (packed.0 >> 6)
                    .try_into()
                    .expect("invalid packed direction"),
                drops: Drops::new::<N>(packed.1)?,
            }
        };

        ply.validate()?;
        Ok(ply)
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

#[cfg(test)]
mod tests {
    use super::*;

    use tak::{Direction, PieceType};

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

    fn test_slot<const N: usize>(hash: ZobristHash, value: usize) -> Option<Slot<N>> {
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

        // When the probe depth is reached, insert should replace the lowest score entry.
        assert!(tt.insert(53, test_entry(1)));
        assert_eq!(
            tt.values,
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
            tt.values,
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
            tt.values,
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
        assert_eq!(tt.get(3), Some(&test_entry(3)));

        // Get works when we have to probe for it.
        assert_eq!(tt.get(33), Some(&test_entry(5)));

        // Get returns none when there's an empty slot.
        assert_eq!(tt.get(2), None);

        // Get returns none when no match is found within the probe range.
        assert_eq!(tt.get(63), None);

        // Get works when probing has to wrap around.
        assert_eq!(tt.get(29), Some(&test_entry(3)));
    }

    #[test]
    fn entry_info() {
        let entry_info = EntryInfo::new(Bound::Exact, 32, 511);
        assert_eq!(entry_info.0, 0xBFFF);
        assert_eq!(entry_info.bound(), Bound::Exact);
        assert_eq!(entry_info.depth(), 32);
        assert_eq!(entry_info.ply_count(), 511);
    }

    #[test]
    fn packed_ply() {
        let ply = Ply::<5>::Place {
            x: 0,
            y: 0,
            piece_type: PieceType::Flatstone,
        };
        let packed: PackedPly = ply.into();
        let unpacked: Ply<5> = packed.try_into().unwrap();
        assert_eq!(packed, PackedPly(0b11000000, 0b00000000));
        assert_eq!(unpacked, ply);

        let ply = Ply::<5>::Place {
            x: 2,
            y: 3,
            piece_type: PieceType::Capstone,
        };
        let packed: PackedPly = ply.into();
        let unpacked: Ply<5> = packed.try_into().unwrap();
        assert_eq!(packed, PackedPly(0b11000000, 0b10010011));
        assert_eq!(unpacked, ply);

        let ply = Ply::<5>::Spread {
            x: 0,
            y: 0,
            direction: Direction::North,
            drops: Drops::new::<5>(1).unwrap(),
        };
        let packed: PackedPly = ply.into();
        let unpacked: Ply<5> = packed.try_into().unwrap();
        assert_eq!(packed, PackedPly(0b00000000, 0b00000001));
        assert_eq!(unpacked, ply);

        let ply = Ply::<5>::Spread {
            x: 4,
            y: 2,
            direction: Direction::West,
            drops: Drops::from_drop_counts::<5>(&[2, 1, 1, 1]).unwrap(),
        };
        let packed: PackedPly = ply.into();
        let unpacked: Ply<5> = packed.try_into().unwrap();
        assert_eq!(packed, PackedPly(0b11100010, 0b00011110));
        assert_eq!(unpacked, ply);
    }
}
