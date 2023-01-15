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

    pub fn dilate(self) -> Self {
        use Direction::*;

        let mut dilation = self;
        dilation |= self << 1 & !edge_masks::<N>()[East as usize] & board_mask::<N>();
        dilation |= self >> 1 & !edge_masks::<N>()[West as usize];
        dilation |= self << N & board_mask::<N>();
        dilation |= self >> N;

        dilation
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

        fn pop_bit<const N: usize>(bitmap: Bitmap<N>) -> Bitmap<N> {
            let remainder = bitmap & (*bitmap - 1);
            bitmap & !remainder
        }

        fn flood_fill<const N: usize>(mut seed: Bitmap<N>, mask: Bitmap<N>) -> Bitmap<N> {
            loop {
                let next = seed.dilate() & mask;
                if next == seed {
                    return seed;
                }
                seed = next;
            }
        }

        let bit = pop_bit(self.seeds);
        let group = flood_fill(bit, self.bitmap);
        self.seeds &= !group;
        self.bitmap &= !group;

        Some(group)
    }
}

pub const fn board_mask<const N: usize>() -> Bitmap<N> {
    const BOARD_MASKS: [u64; 9] = [
        0,
        0,
        0,
        0x01FF,
        0xFFFF,
        0x01FFFFFF,
        0x0FFFFFFFFF,
        0x01FFFFFFFFFFFF,
        0xFFFFFFFFFFFFFFFF,
    ];

    Bitmap::new(BOARD_MASKS[N])
}

pub const fn edge_masks<const N: usize>() -> [Bitmap<N>; 4] {
    const EDGE_MASKS: [[u64; 4]; 9] = [
        [0; 4],
        [0; 4],
        [0; 4],
        [0x01C0, 0x0049, 0x0007, 0x0124],
        [0xF000, 0x1111, 0x000F, 0x8888],
        [0x01F00000, 0x00108421, 0x0000001F, 0x01084210],
        [0x0FC0000000, 0x0041041041, 0x000000003F, 0x0820820820],
        [
            0x01FC0000000000,
            0x00040810204081,
            0x0000000000007F,
            0x01020408102040,
        ],
        [
            0xFF00000000000000,
            0x0101010101010101,
            0x00000000000000FF,
            0x8080808080808080,
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
        BitAnd, BitAndAssign, BitOr, BitOrAssign, Deref, DerefMut, Not, Shl, ShlAssign, Shr,
        ShrAssign,
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

    impl_bitmap_binary_ops!([(BitAnd, bitand), (BitOr, bitor)]);

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

    impl_bitmap_assign_op!([(BitAndAssign, bitand_assign), (BitOrAssign, bitor_assign)]);

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
        assert_eq!(b, Bitmap::new(0x10));

        let mut b = Bitmap::<5>::default();
        b.set(1, 0);
        assert_eq!(b, Bitmap::new(0x08));

        let mut b = Bitmap::<5>::default();
        b.set(1, 1);
        assert_eq!(b, Bitmap::new(0x0100));

        let mut b = Bitmap::<5>::default();
        b.set(4, 4);
        assert_eq!(b, Bitmap::new(0x100000));
    }

    #[test]
    fn clear() {
        let mut b = Bitmap::<5>::new(0xFFFFFFFFFFFFFFFF);
        b.clear(0, 0);
        assert_eq!(b, Bitmap::new(0xFFFFFFFFFFFFFFEF));

        let mut b = Bitmap::<5>::new(0xFFFFFFFFFFFFFFFF);
        b.clear(1, 0);
        assert_eq!(b, Bitmap::new(0xFFFFFFFFFFFFFFF7));

        let mut b = Bitmap::<5>::new(0xFFFFFFFFFFFFFFFF);
        b.clear(1, 1);
        assert_eq!(b, Bitmap::new(0xFFFFFFFFFFFFFEFF));

        let mut b = Bitmap::<5>::new(0xFFFFFFFFFFFFFFFF);
        b.clear(4, 4);
        assert_eq!(b, Bitmap::new(0xFFFFFFFFFFEFFFFF));
    }

    #[test]
    fn get() {
        let b = Bitmap::<5>::new(0b0000000110001001010101000);
        assert!(!b.get(0, 0));
        assert!(b.get(1, 0));
        assert!(b.get(0, 1));
        assert!(b.get(2, 1));
        assert!(b.get(2, 2));
        assert!(!b.get(3, 2));
    }

    #[test]
    fn dilate() {
        let b = Bitmap::<5>::new(0b0000000000001000000000000);
        assert_eq!(b.dilate(), Bitmap::new(0b0000000100011100010000000),);

        let b = Bitmap::<5>::new(0b1000100000000000000010001);
        assert_eq!(b.dilate(), Bitmap::new(0b1101110001000001000111011),);

        let b = Bitmap::<5>::new(0b0000000100011100010000000);
        assert_eq!(b.dilate(), Bitmap::new(0b0010001110111110111000100),);
    }

    #[test]
    fn groups() {
        let mut g = Bitmap::<5>::new(0b1110011010001100011111000).groups();

        assert_eq!(g.next(), Some(Bitmap::new(0b0000000000000000000011000)),);

        assert_eq!(g.next(), Some(Bitmap::new(0b0000000010001100011100000)),);

        assert_eq!(g.next(), Some(Bitmap::new(0b1110011000000000000000000)),);

        assert_eq!(g.next(), None);
    }

    #[test]
    fn width() {
        let b = Bitmap::<5>::new(0b0000001100011100100001000);
        assert_eq!(b.width(), 3);
    }

    #[test]
    fn height() {
        let b = Bitmap::<5>::new(0b0000001100011100100001000);
        assert_eq!(b.height(), 4);
    }

    #[test]
    fn edge_masks_are_correct() {
        fn all_edges<const N: usize>() -> Bitmap<N> {
            use Direction::*;
            let edges = edge_masks();
            edges[North as usize]
                | edges[East as usize]
                | edges[South as usize]
                | edges[West as usize]
        }

        assert_eq!(
            format!("{:?}", all_edges::<3>()),
            "111/101/111",
            "3s is wrong"
        );
        assert_eq!(
            format!("{:?}", all_edges::<4>()),
            "1111/1001/1001/1111",
            "4s is wrong"
        );
        assert_eq!(
            format!("{:?}", all_edges::<5>()),
            "11111/10001/10001/10001/11111",
            "5s is wrong"
        );
        assert_eq!(
            format!("{:?}", all_edges::<6>()),
            "111111/100001/100001/100001/100001/111111",
            "6s is wrong"
        );
        assert_eq!(
            format!("{:?}", all_edges::<7>()),
            "1111111/1000001/1000001/1000001/1000001/1000001/1111111",
            "7s is wrong"
        );
        assert_eq!(
            format!("{:?}", all_edges::<8>()),
            "11111111/10000001/10000001/10000001/10000001/10000001/10000001/11111111",
            "8s is wrong"
        );
    }
}
