use std::{fmt, io, mem};

use tak::{edge_masks, Bitmap, Direction, Drops, Ply, PlyError};

pub(crate) mod bag;

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
///     MSB - d d x x x y y y , d d d d d d d d - LSB
/// ```
/// These patterns are distinguishable because the "magic" value
/// cannot be interpreted as a valid spread; it would represent a
/// spread West from (0, 0), which is impossible.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct PackedPly(u8, u8);

impl PackedPly {
    pub(crate) fn to_bits(self) -> u16 {
        debug_assert_eq!(mem::size_of::<Self>(), mem::size_of::<u16>(),);

        unsafe { mem::transmute(self) }
    }

    pub(crate) fn from_bits(value: u16) -> Self {
        debug_assert_eq!(mem::size_of::<Self>(), mem::size_of::<u16>(),);

        unsafe { mem::transmute(value) }
    }
}

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

impl fmt::Debug for PackedPly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ply: Ply<8> = (*self).try_into().unwrap();
        write!(f, "{ply:?}")
    }
}

/// Returns a map filled with all single locations that would complete a road.
pub(crate) fn placement_threat_map<const N: usize>(
    road_pieces: Bitmap<N>,
    blocking_pieces: Bitmap<N>,
) -> Bitmap<N> {
    use Direction::*;

    let edges = edge_masks();

    let left_pieces = edges[West as usize].flood_fill(road_pieces);
    let right_pieces = edges[East as usize].flood_fill(road_pieces);
    let horizontal_threats = (left_pieces.dilate() | edges[West as usize])
        & (right_pieces.dilate() | edges[East as usize]);

    let top_pieces = edges[North as usize].flood_fill(road_pieces);
    let bottom_pieces = edges[South as usize].flood_fill(road_pieces);
    let vertical_threats = (top_pieces.dilate() | edges[North as usize])
        & (bottom_pieces.dilate() | edges[South as usize]);

    (horizontal_threats | vertical_threats) & !blocking_pieces
}

/// This provides a polyfill for f32::next_up() and f32::next_down() until those
/// become stable in the std.
pub(crate) trait Neighbors {
    fn next_up(self) -> Self;
    fn next_down(self) -> Self;

    fn next_n_up(mut self, n: usize) -> Self
    where
        Self: Sized,
    {
        for _ in 0..n {
            self = self.next_up();
        }
        self
    }

    fn next_n_down(mut self, n: usize) -> Self
    where
        Self: Sized,
    {
        for _ in 0..n {
            self = self.next_down();
        }
        self
    }
}

/// These implementations come from the reference implementations in https://rust-lang.github.io/rfcs/3173-float-next-up-down.html
impl Neighbors for f32 {
    fn next_up(self) -> Self {
        const TINY_BITS: u32 = 0x1; // Smallest positive f32.
        const CLEAR_SIGN_MASK: u32 = 0x7fff_ffff;

        let bits = self.to_bits();
        if self.is_nan() || bits == Self::INFINITY.to_bits() {
            return self;
        }

        let abs = bits & CLEAR_SIGN_MASK;
        let next_bits = if abs == 0 {
            TINY_BITS
        } else if bits == abs {
            bits + 1
        } else {
            bits - 1
        };
        Self::from_bits(next_bits)
    }

    fn next_down(self) -> Self {
        const NEG_TINY_BITS: u32 = 0x8000_0001; // Smallest (in magnitude) negative f32.
        const CLEAR_SIGN_MASK: u32 = 0x7fff_ffff;

        let bits = self.to_bits();
        if self.is_nan() || bits == Self::NEG_INFINITY.to_bits() {
            return self;
        }

        let abs = bits & CLEAR_SIGN_MASK;
        let next_bits = if abs == 0 {
            NEG_TINY_BITS
        } else if bits == abs {
            bits - 1
        } else {
            bits + 1
        };
        Self::from_bits(next_bits)
    }
}

pub trait Sender<T> {
    fn send(&self, value: T) -> Result<(), io::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    use tak::PieceType;

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

    #[test]
    fn placement_threat_maps_are_correct() {
        let b: Bitmap<5> = 0b01000_11110_01000_00000_01000.into();

        let t = placement_threat_map(b, 0.into());
        assert_eq!(t, 0b00000_00001_00000_01000_00000.into());

        let t = placement_threat_map(b, 0b01000_11111_01000_00000_01000.into());
        assert_eq!(t, 0b00000_00000_00000_01000_00000.into());

        let b: Bitmap<6> = 0b001000_111110_101010_010101_011111_000100.into();

        let t = placement_threat_map(b, 0.into());
        assert_eq!(t, 0b000000_000001_010101_101010_100000_000000.into());
    }
}
