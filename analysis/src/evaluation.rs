use std::fmt;

use tak::{Color, Resolution, State};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Evaluation(EvalType);

impl Evaluation {
    pub fn zero() -> Self {
        0.into()
    }

    pub fn win() -> Self {
        100_000.into()
    }

    pub fn lose() -> Self {
        -Self::win()
    }

    pub fn max() -> Self {
        Self(EvalType::MAX - 1)
    }

    pub fn min() -> Self {
        Self(EvalType::MIN + 1)
    }
}

type EvalType = i32;

struct Weights {
    flatstone: EvalType,
    standing_stone: EvalType,
    capstone: EvalType,
}

const WEIGHT: Weights = Weights {
    flatstone: 400,
    standing_stone: 200,
    capstone: 300,
};

pub(crate) fn evaluate<const N: usize>(state: &State<N>) -> Evaluation {
    let next_color = if state.ply_count % 2 == 0 {
        Color::White
    } else {
        Color::Black
    };

    match state.resolution() {
        None => (),
        Some(Resolution::Road(color)) | Some(Resolution::Flats { color, .. }) => {
            if color == next_color {
                return Evaluation::win() - state.ply_count as i32;
            } else {
                return Evaluation::lose() + state.ply_count as i32;
            }
        }
        Some(Resolution::Draw) => return Evaluation::zero(),
    }

    let m = &state.metadata;

    let mut p1_eval = Evaluation::zero();
    p1_eval += (m.p1_pieces & m.flatstones).count_ones() as EvalType * WEIGHT.flatstone;
    p1_eval += (m.p1_pieces & m.standing_stones).count_ones() as EvalType * WEIGHT.standing_stone;
    p1_eval += (m.p1_pieces & m.capstones).count_ones() as EvalType * WEIGHT.capstone;

    let mut p2_eval = Evaluation::zero();
    p2_eval += (m.p2_pieces & m.flatstones).count_ones() as EvalType * WEIGHT.flatstone;
    p2_eval += (m.p2_pieces & m.standing_stones).count_ones() as EvalType * WEIGHT.standing_stone;
    p2_eval += (m.p2_pieces & m.capstones).count_ones() as EvalType * WEIGHT.capstone;

    match next_color {
        Color::White => p1_eval - p2_eval,
        Color::Black => p2_eval - p1_eval,
    }
}

impl From<EvalType> for Evaluation {
    fn from(value: EvalType) -> Self {
        Self(value)
    }
}

impl fmt::Display for Evaluation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

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
