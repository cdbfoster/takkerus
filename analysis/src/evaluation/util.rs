use tak::{edge_masks, Bitmap, Direction};

const WIN: EvalType = 100_000;
const WIN_THRESHOLD: EvalType = 99_000;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Evaluation(pub(super) EvalType);

impl Evaluation {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(EvalType::MAX - 1);
    pub const MIN: Self = Self(EvalType::MIN + 1);
    pub const WIN: Self = Self(WIN);
    pub const LOSE: Self = Self(-WIN);

    pub fn is_terminal(self) -> bool {
        self.0.abs() > WIN_THRESHOLD
    }
}

pub(super) type EvalType = i32;

mod ops {
    use super::*;
    use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

    macro_rules! impl_evaluation_binary_ops {
        ($(($op:ident, $fn:ident)),+) => {
            $(
                impl $op<EvalType> for Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: EvalType) -> Self::Output {
                        Evaluation(self.0.$fn(other))
                    }
                }

                impl $op<&EvalType> for Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: &EvalType) -> Self::Output {
                        Evaluation(self.0.$fn(other))
                    }
                }

                impl $op<EvalType> for &Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: EvalType) -> Self::Output {
                        Evaluation(self.0.$fn(other))
                    }
                }

                impl $op<&EvalType> for &Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: &EvalType) -> Self::Output {
                        Evaluation(self.0.$fn(other))
                    }
                }

                impl $op<Evaluation> for Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: Evaluation) -> Self::Output {
                        Evaluation(self.0.$fn(other.0))
                    }
                }

                impl $op<&Evaluation> for Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: &Evaluation) -> Self::Output {
                        Evaluation(self.0.$fn(other.0))
                    }
                }

                impl $op<Evaluation> for &Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: Evaluation) -> Self::Output {
                        Evaluation(self.0.$fn(other.0))
                    }
                }

                impl $op<&Evaluation> for &Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: &Evaluation) -> Self::Output {
                        Evaluation(self.0.$fn(other.0))
                    }
                }
            )+
        };
    }

    impl_evaluation_binary_ops!((Add, add), (Div, div), (Mul, mul), (Sub, sub));

    macro_rules! impl_evaluation_assign_ops {
        ($(($op:ident, $fn:ident)),+) => {
            $(
                impl $op<EvalType> for Evaluation {
                    fn $fn(&mut self, other: EvalType) {
                        self.0.$fn(other)
                    }
                }

                impl $op<&EvalType> for Evaluation {
                    fn $fn(&mut self, other: &EvalType) {
                        self.0.$fn(other)
                    }
                }

                impl $op<Evaluation> for Evaluation {
                    fn $fn(&mut self, other: Evaluation) {
                        self.0.$fn(other.0)
                    }
                }

                impl $op<&Evaluation> for Evaluation {
                    fn $fn(&mut self, other: &Evaluation) {
                        self.0.$fn(other.0)
                    }
                }
            )+
        };
    }

    impl_evaluation_assign_ops!(
        (AddAssign, add_assign),
        (DivAssign, div_assign),
        (MulAssign, mul_assign),
        (SubAssign, sub_assign)
    );

    impl Neg for Evaluation {
        type Output = Evaluation;

        fn neg(self) -> Self::Output {
            Evaluation(-self.0)
        }
    }
}

/// Returns two maps, filled with all single locations that would complete a road
/// horizontally and vertically, respectively.
pub(crate) fn placement_threat_maps<const N: usize>(
    all_pieces: Bitmap<N>,
    road_pieces: Bitmap<N>,
) -> (Bitmap<N>, Bitmap<N>) {
    use Direction::*;

    let other_pieces = all_pieces & !road_pieces;

    let edges = edge_masks();

    let left_pieces = edges[West as usize].flood_fill(road_pieces);
    let right_pieces = edges[East as usize].flood_fill(road_pieces);
    let horizontal_threats = (left_pieces.dilate() | edges[West as usize])
        & (right_pieces.dilate() | edges[East as usize])
        & !other_pieces;

    let top_pieces = edges[North as usize].flood_fill(road_pieces);
    let bottom_pieces = edges[South as usize].flood_fill(road_pieces);
    let vertical_threats = (top_pieces.dilate() | edges[North as usize])
        & (bottom_pieces.dilate() | edges[South as usize])
        & !other_pieces;

    (horizontal_threats, vertical_threats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_threat_maps_are_correct() {
        let b: Bitmap<5> = 0b0100011110010000000001000.into();

        let (h, v) = placement_threat_maps(0.into(), b);
        assert_eq!(h, 0b0000000001000000000000000.into());
        assert_eq!(v, 0b0000000000000000100000000.into());

        let (h, v) = placement_threat_maps(0b0100011111010000000001000.into(), b);
        assert_eq!(h, 0.into());
        assert_eq!(v, 0b0000000000000000100000000.into());

        let b: Bitmap<6> = 0b001000111110101010010101011111000100.into();

        let (h, v) = placement_threat_maps(0.into(), b);
        assert_eq!(h, 0b000000000001010101101010100000000000.into());
        assert_eq!(v, 0b000000000000010101101010000000000000.into());
    }
}
