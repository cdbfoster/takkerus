#![allow(clippy::unusual_byte_groupings)]

use std::fmt;

use crate::ply::Direction;

#[repr(transparent)]
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct Bitmap<const N: usize>(u64);

impl<const N: usize> Bitmap<N> {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn set(&mut self, x: usize, y: usize) {
        debug_assert!(x < N);
        debug_assert!(y < N);
        *self |= 1 << ((N - 1 - x) + y * N);
    }

    pub fn clear(&mut self, x: usize, y: usize) {
        debug_assert!(x < N);
        debug_assert!(y < N);
        *self &= !(1 << ((N - 1 - x) + y * N));
    }

    pub fn get(self, x: usize, y: usize) -> bool {
        debug_assert!(x < N);
        debug_assert!(y < N);
        *self & (1 << ((N - 1 - x) + y * N)) > 0
    }

    pub fn coordinates(self) -> (usize, usize) {
        debug_assert_eq!(self.count_ones(), 1);

        let index = self.trailing_zeros();
        let y = index as usize / N;
        let x = N - 1 - index as usize % N;

        (x, y)
    }

    pub fn dilate(self) -> Self {
        use Direction::*;

        let mut dilation = self;
        dilation |= self << 1 & !edge_masks()[East as usize] & board_mask();
        dilation |= self >> 1 & !edge_masks()[West as usize];
        dilation |= self << N & board_mask();
        dilation |= self >> N;

        dilation
    }

    pub fn flood_fill(self, mask: Self) -> Self {
        let mut seed = self & mask;

        loop {
            let next = seed.dilate() & mask;
            if next == seed {
                return seed;
            }
            seed = next;
        }
    }

    pub fn groups(self) -> GroupIter<N> {
        GroupIter {
            seeds: self,
            bitmap: self,
        }
    }

    pub fn groups_from(self, seeds: Bitmap<N>) -> GroupIter<N> {
        assert_eq!(
            seeds & !self,
            0.into(),
            "provided seeds are not part of the bitmap"
        );
        GroupIter {
            seeds,
            bitmap: self,
        }
    }

    pub fn width(self) -> usize {
        let mut row_mask = edge_masks::<N>()[Direction::North as usize];
        let mut row_aggregate = Bitmap::default();
        for i in 0..N {
            let row = self & row_mask;
            row_aggregate |= row << (i * N);
            row_mask >>= N;
        }
        row_aggregate.count_ones() as usize
    }

    pub fn height(self) -> usize {
        let mut column_mask = edge_masks::<N>()[Direction::West as usize];
        let mut column_aggregate = Bitmap::default();
        for i in 0..N {
            let column = self & column_mask;
            column_aggregate |= column << i;
            column_mask >>= 1;
        }
        column_aggregate.count_ones() as usize
    }

    pub fn lowest_bit(self: Bitmap<N>) -> Bitmap<N> {
        let remainder = self & (*self - 1);
        self & !remainder
    }

    pub fn bits(self) -> BitIter<N> {
        BitIter { bitmap: self }
    }
}

impl<const N: usize> fmt::Debug for Bitmap<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let row_mask = 0xFFFFFFFFFFFFFFFF >> (64 - N);
        let column_mask = 1;
        for y in 1..=N {
            if y > 1 {
                write!(f, "/")?;
            }
            let row = (**self >> (N * N - y * N)) & row_mask;
            for x in 1..=N {
                let column = (row >> (N - x)) & column_mask;
                write!(f, "{column}")?;
            }
        }
        Ok(())
    }
}

impl<const N: usize> From<u64> for Bitmap<N> {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

pub struct GroupIter<const N: usize> {
    seeds: Bitmap<N>,
    bitmap: Bitmap<N>,
}

impl<const N: usize> Iterator for GroupIter<N> {
    type Item = Bitmap<N>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.seeds == 0 {
            return None;
        }

        let bit = self.seeds.lowest_bit();
        let group = bit.flood_fill(self.bitmap);
        self.seeds &= !group;
        self.bitmap &= !group;

        Some(group)
    }
}

pub struct BitIter<const N: usize> {
    bitmap: Bitmap<N>,
}

impl<const N: usize> Iterator for BitIter<N> {
    type Item = Bitmap<N>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.bitmap == 0 {
            return None;
        }

        let remainder = self.bitmap & (*self.bitmap - 1);
        let bit = self.bitmap & !remainder;
        self.bitmap = remainder;
        Some(bit)
    }
}

pub const fn board_mask<const N: usize>() -> Bitmap<N> {
    const BOARD_MASKS: [u64; 9] = [
        0,
        0,
        0,
        0b111_111_111,
        0b1111_1111_1111_1111,
        0b11111_11111_11111_11111_11111,
        0b111111_111111_111111_111111_111111_111111,
        0b1111111_1111111_1111111_1111111_1111111_1111111_1111111,
        0b11111111_11111111_11111111_11111111_11111111_11111111_11111111_11111111,
    ];

    Bitmap::new(BOARD_MASKS[N])
}

pub const fn center_mask<const N: usize>() -> Bitmap<N> {
    const CENTER_MASKS: [u64; 9] = [
        0,
        0,
        0,
        0b000_010_000,
        0b0000_0110_0110_0000,
        0b00000_00000_00100_00000_00000,
        0b000000_000000_001100_001100_000000_000000,
        0b0000000_0000000_0000000_0001000_0000000_0000000_0000000,
        0b00000000_00000000_00000000_00011000_00011000_00000000_00000000_00000000,
    ];

    Bitmap::new(CENTER_MASKS[N])
}

pub const fn edge_masks<const N: usize>() -> [Bitmap<N>; 4] {
    const EDGE_MASKS: [[u64; 4]; 9] = [
        [0; 4],
        [0; 4],
        [0; 4],
        [0b111_000_000, 0b001_001_001, 0b000_000_111, 0b100_100_100],
        [
            0b1111_0000_0000_0000,
            0b0001_0001_0001_0001,
            0b0000_0000_0000_1111,
            0b1000_1000_1000_1000,
        ],
        [
            0b11111_00000_00000_00000_00000,
            0b00001_00001_00001_00001_00001,
            0b00000_00000_00000_00000_11111,
            0b10000_10000_10000_10000_10000,
        ],
        [
            0b111111_000000_000000_000000_000000_000000,
            0b000001_000001_000001_000001_000001_000001,
            0b000000_000000_000000_000000_000000_111111,
            0b100000_100000_100000_100000_100000_100000,
        ],
        [
            0b1111111_0000000_0000000_0000000_0000000_0000000_0000000,
            0b0000001_0000001_0000001_0000001_0000001_0000001_0000001,
            0b0000000_0000000_0000000_0000000_0000000_0000000_1111111,
            0b1000000_1000000_1000000_1000000_1000000_1000000_1000000,
        ],
        [
            0b11111111_00000000_00000000_00000000_00000000_00000000_00000000_00000000,
            0b00000001_00000001_00000001_00000001_00000001_00000001_00000001_00000001,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_11111111,
            0b10000000_10000000_10000000_10000000_10000000_10000000_10000000_10000000,
        ],
    ];

    use Direction::*;

    [
        Bitmap::new(EDGE_MASKS[N][North as usize]),
        Bitmap::new(EDGE_MASKS[N][East as usize]),
        Bitmap::new(EDGE_MASKS[N][South as usize]),
        Bitmap::new(EDGE_MASKS[N][West as usize]),
    ]
}

mod ops {
    use super::*;

    use std::ops::{
        BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, DerefMut, Not, Shl,
        ShlAssign, Shr, ShrAssign,
    };

    impl<const N: usize> Deref for Bitmap<N> {
        type Target = u64;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<const N: usize> DerefMut for Bitmap<N> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    macro_rules! impl_pod_binary_op {
        ($op:ident, $fn:ident, [$($t:ident),+]) => {
            $(
                impl<const N: usize> $op<$t> for Bitmap<N> {
                    type Output = Bitmap<N>;

                    fn $fn(self, value: $t) -> Self::Output {
                        Bitmap::new(self.0.$fn(value))
                    }
                }

                impl<const N: usize> $op<&$t> for Bitmap<N> {
                    type Output = Bitmap<N>;

                    fn $fn(self, value: &$t) -> Self::Output {
                        Bitmap::new(self.0.$fn(value))
                    }
                }

                impl<const N: usize> $op<$t> for &Bitmap<N> {
                    type Output = Bitmap<N>;

                    fn $fn(self, value: $t) -> Self::Output {
                        Bitmap::new(self.0.$fn(value))
                    }
                }

                impl<const N: usize> $op<&$t> for &Bitmap<N> {
                    type Output = Bitmap<N>;

                    fn $fn(self, value: &$t) -> Self::Output {
                        Bitmap::new(self.0.$fn(value))
                    }
                }
            )+
        };
    }

    impl_pod_binary_op!(BitAnd, bitand, [u64]);
    impl_pod_binary_op!(BitOr, bitor, [u64]);
    impl_pod_binary_op!(BitXor, bitxor, [u64]);
    impl_pod_binary_op!(
        Shl,
        shl,
        [i8, i16, i32, i64, isize, u8, u16, u32, u64, usize]
    );
    impl_pod_binary_op!(
        Shr,
        shr,
        [i8, i16, i32, i64, isize, u8, u16, u32, u64, usize]
    );

    macro_rules! impl_pod_assign_op {
        ($op:ident, $fn:ident, [$($t:ident),+]) => {
            $(
                impl<const N: usize> $op<$t> for Bitmap<N> {
                    fn $fn(&mut self, value: $t) {
                        self.0.$fn(value);
                    }
                }

                impl<const N: usize> $op<&$t> for Bitmap<N> {
                    fn $fn(&mut self, value: &$t) {
                        self.0.$fn(value);
                    }
                }
            )+
        };
    }

    impl_pod_assign_op!(BitAndAssign, bitand_assign, [u64]);
    impl_pod_assign_op!(BitOrAssign, bitor_assign, [u64]);
    impl_pod_assign_op!(BitXorAssign, bitxor_assign, [u64]);
    impl_pod_assign_op!(
        ShlAssign,
        shl_assign,
        [i8, i16, i32, i64, isize, u8, u16, u32, u64, usize]
    );
    impl_pod_assign_op!(
        ShrAssign,
        shr_assign,
        [i8, i16, i32, i64, isize, u8, u16, u32, u64, usize]
    );

    macro_rules! impl_bitmap_binary_ops {
        ([$(($op:ident, $fn:ident)),+]) => {
            $(
                impl<const N: usize> $op<Bitmap<N>> for Bitmap<N> {
                    type Output = Bitmap<N>;

                    fn $fn(self, value: Bitmap<N>) -> Self::Output {
                        Bitmap::new(self.0.$fn(value.0))
                    }
                }

                impl<const N: usize> $op<&Bitmap<N>> for Bitmap<N> {
                    type Output = Bitmap<N>;

                    fn $fn(self, value: &Bitmap<N>) -> Self::Output {
                        Bitmap::new(self.0.$fn(value.0))
                    }
                }

                impl<const N: usize> $op<Bitmap<N>> for &Bitmap<N> {
                    type Output = Bitmap<N>;

                    fn $fn(self, value: Bitmap<N>) -> Self::Output {
                        Bitmap::new(self.0.$fn(value.0))
                    }
                }

                impl<const N: usize> $op<&Bitmap<N>> for &Bitmap<N> {
                    type Output = Bitmap<N>;

                    fn $fn(self, value: &Bitmap<N>) -> Self::Output {
                        Bitmap::new(self.0.$fn(value.0))
                    }
                }
            )+
        };
    }

    impl_bitmap_binary_ops!([(BitAnd, bitand), (BitOr, bitor), (BitXor, bitxor)]);

    macro_rules! impl_bitmap_assign_op {
        ([$(($op:ident, $fn:ident)),+]) => {
            $(
                impl<const N: usize> $op<Bitmap<N>> for Bitmap<N> {
                    fn $fn(&mut self, value: Bitmap<N>) {
                        self.0.$fn(value.0);
                    }
                }

                impl<const N: usize> $op<&Bitmap<N>> for Bitmap<N> {
                    fn $fn(&mut self, value: &Bitmap<N>) {
                        self.0.$fn(value.0);
                    }
                }
            )+
        };
    }

    impl_bitmap_assign_op!([
        (BitAndAssign, bitand_assign),
        (BitOrAssign, bitor_assign),
        (BitXorAssign, bitxor_assign)
    ]);

    impl<const N: usize> Not for Bitmap<N> {
        type Output = Bitmap<N>;

        fn not(self) -> Self::Output {
            Bitmap::new(!self.0)
        }
    }

    impl<const N: usize> Not for &Bitmap<N> {
        type Output = Bitmap<N>;

        fn not(self) -> Self::Output {
            Bitmap::new(!self.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set() {
        let mut b = Bitmap::<5>::default();
        b.set(0, 0);
        assert_eq!(b, Bitmap::new(0b00000_00000_00000_00000_10000));

        let mut b = Bitmap::<5>::default();
        b.set(1, 0);
        assert_eq!(b, Bitmap::new(0b00000_00000_00000_00000_01000));

        let mut b = Bitmap::<5>::default();
        b.set(1, 1);
        assert_eq!(b, Bitmap::new(0b00000_00000_00000_01000_00000));

        let mut b = Bitmap::<5>::default();
        b.set(4, 4);
        assert_eq!(b, Bitmap::new(0b00001_00000_00000_00000_00000));
    }

    #[test]
    fn clear() {
        let mut b = Bitmap::<5>::new(0b11111_11111_11111_11111_11111);
        b.clear(0, 0);
        assert_eq!(b, Bitmap::new(0b11111_11111_11111_11111_01111));

        let mut b = Bitmap::<5>::new(0b11111_11111_11111_11111_11111);
        b.clear(1, 0);
        assert_eq!(b, Bitmap::new(0b11111_11111_11111_11111_10111));

        let mut b = Bitmap::<5>::new(0b11111_11111_11111_11111_11111);
        b.clear(1, 1);
        assert_eq!(b, Bitmap::new(0b11111_11111_11111_10111_11111));

        let mut b = Bitmap::<5>::new(0b11111_11111_11111_11111_11111);
        b.clear(4, 4);
        assert_eq!(b, Bitmap::new(0b11110_11111_11111_11111_11111));
    }

    #[test]
    fn get() {
        let b = Bitmap::<5>::new(0b00000_00110_00100_10101_01000);
        assert!(!b.get(0, 0));
        assert!(b.get(1, 0));
        assert!(b.get(0, 1));
        assert!(b.get(2, 1));
        assert!(b.get(2, 2));
        assert!(!b.get(3, 2));
    }

    #[test]
    fn coordinates() {
        assert_eq!(Bitmap::<3>::new(0b000_000_001).coordinates(), (2, 0));
        assert_eq!(Bitmap::<3>::new(0b000_010_000).coordinates(), (1, 1));
        assert_eq!(Bitmap::<3>::new(0b000_100_000).coordinates(), (0, 1));
        assert_eq!(Bitmap::<3>::new(0b010_000_000).coordinates(), (1, 2));
    }

    #[test]
    fn dilate() {
        let b = Bitmap::<5>::new(0b00000_00000_00100_00000_00000);
        assert_eq!(b.dilate(), 0b00000_00100_01110_00100_00000.into());

        let b = Bitmap::<5>::new(0b10001_00000_00000_00000_10001);
        assert_eq!(b.dilate(), 0b11011_10001_00000_10001_11011.into());

        let b = Bitmap::<5>::new(0b00000_00100_01110_00100_00000);
        assert_eq!(b.dilate(), 0b00100_01110_11111_01110_00100.into());
    }

    #[test]
    fn groups() {
        let mut g = Bitmap::<5>::new(0b11100_11010_00110_00111_11000).groups();

        assert_eq!(g.next(), Some(0b00000_00000_00000_00000_11000.into()));
        assert_eq!(g.next(), Some(0b00000_00010_00110_00111_00000.into()));
        assert_eq!(g.next(), Some(0b11100_11000_00000_00000_00000.into()));
        assert_eq!(g.next(), None);
    }

    #[test]
    fn width() {
        let b = Bitmap::<5>::new(0b00000_01100_01110_01000_01000);
        assert_eq!(b.width(), 3);
    }

    #[test]
    fn height() {
        let b = Bitmap::<5>::new(0b00000_01100_01110_01000_01000);
        assert_eq!(b.height(), 4);
    }

    #[test]
    fn bits() {
        let mut b = Bitmap::<3>::new(0b010_110_001).bits();

        assert_eq!(b.next(), Some(0b000_000_001.into()));
        assert_eq!(b.next(), Some(0b000_010_000.into()));
        assert_eq!(b.next(), Some(0b000_100_000.into()));
        assert_eq!(b.next(), Some(0b010_000_000.into()));
        assert_eq!(b.next(), None);
    }
}
