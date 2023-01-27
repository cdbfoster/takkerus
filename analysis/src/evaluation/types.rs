use std::fmt;

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
