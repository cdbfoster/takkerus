use std::fmt;

use tak::{Bitmap, Color, Metadata, Resolution, State};

const WIN: EvalType = 100_000;
const WIN_THRESHOLD: EvalType = 99_000;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Evaluation(EvalType);

impl Evaluation {
    pub fn zero() -> Self {
        0.into()
    }

    pub fn win() -> Self {
        WIN.into()
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

    pub fn is_win(self) -> bool {
        self.0.abs() > WIN_THRESHOLD
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

pub fn evaluate<const N: usize>(state: &State<N>) -> Evaluation {
    use Color::*;

    let to_move = if state.ply_count % 2 == 0 {
        Color::White
    } else {
        Color::Black
    };

    match state.resolution() {
        None => (),
        Some(Resolution::Road(color)) | Some(Resolution::Flats { color, .. }) => {
            if color == to_move {
                return Evaluation::win() - state.ply_count as i32;
            } else {
                return Evaluation::lose() + state.ply_count as i32;
            }
        }
        Some(Resolution::Draw) => return Evaluation::zero() - state.ply_count as i32,
    }

    let m = &state.metadata;

    let mut p1_eval = Evaluation::zero();
    let mut p2_eval = Evaluation::zero();

    // Material
    p1_eval += evaluate_material(m, m.p1_pieces);
    p2_eval += evaluate_material(m, m.p2_pieces);

    match to_move {
        White => p1_eval - p2_eval,
        Black => p2_eval - p1_eval,
    }
}

fn evaluate_material<const N: usize>(m: &Metadata<N>, pieces: Bitmap<N>) -> EvalType {
    let mut eval = 0;
    eval += (pieces & m.flatstones).count_ones() as EvalType * WEIGHT.flatstone;
    eval += (pieces & m.standing_stones).count_ones() as EvalType * WEIGHT.standing_stone;
    eval += (pieces & m.capstones).count_ones() as EvalType * WEIGHT.capstone;
    eval
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material() {
        let state: State<6> = "x6/x4,2,1/x2,2,2C,1,2/x2,2,x,1,1/x5,1/x6 1 6"
            .parse()
            .unwrap();
        assert_eq!(
            evaluate_material(&state.metadata, state.metadata.p1_pieces),
            5 * WEIGHT.flatstone,
        );
        assert_eq!(
            evaluate_material(&state.metadata, state.metadata.p2_pieces),
            4 * WEIGHT.flatstone + 1 * WEIGHT.capstone,
        );

        let state: State<6> = "x2,21,122,1121S,112S/1S,x,1112,x,2S,x/112C,2S,x,1222221C,2,x/2,x2,1,2121S,x/112,1112111112S,x3,221S/2,2,x2,21,2 1 56".parse().unwrap();
        assert_eq!(
            evaluate_material(&state.metadata, state.metadata.p1_pieces),
            3 * WEIGHT.flatstone + 4 * WEIGHT.standing_stone + 1 * WEIGHT.capstone,
        );
        assert_eq!(
            evaluate_material(&state.metadata, state.metadata.p2_pieces),
            8 * WEIGHT.flatstone + 4 * WEIGHT.standing_stone + 1 * WEIGHT.capstone,
        );
    }
}
