#![allow(unstable_name_collisions)]

use std::fmt;

use crate::util::Neighbors;

const WIN: f32 = 1.1;
const WIN_THRESHOLD: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct Evaluation(f32);

impl Evaluation {
    pub const ZERO: Self = Self(0.0);
    pub const MAX: Self = Self(f32::MAX);
    pub const MIN: Self = Self(f32::MIN);
    pub const WIN: Self = Self(WIN);
    pub const LOSS: Self = Self(-WIN);

    pub fn is_terminal(self) -> bool {
        self.0.abs() > WIN_THRESHOLD
    }
}

mod ops {
    use super::*;
    use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

    macro_rules! impl_evaluation_binary_ops {
        ($(($op:ident, $fn:ident)),+) => {
            $(
                impl $op<f32> for Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: f32) -> Self::Output {
                        Evaluation(self.0.$fn(other))
                    }
                }

                impl $op<&f32> for Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: &f32) -> Self::Output {
                        Evaluation(self.0.$fn(other))
                    }
                }

                impl $op<f32> for &Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: f32) -> Self::Output {
                        Evaluation(self.0.$fn(other))
                    }
                }

                impl $op<&f32> for &Evaluation {
                    type Output = Evaluation;

                    fn $fn(self, other: &f32) -> Self::Output {
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
                impl $op<f32> for Evaluation {
                    fn $fn(&mut self, other: f32) {
                        self.0.$fn(other)
                    }
                }

                impl $op<&f32> for Evaluation {
                    fn $fn(&mut self, other: &f32) {
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

    impl Eq for Evaluation {}

    impl Neighbors for Evaluation {
        fn next_up(self) -> Self {
            Self(self.0.next_up())
        }

        fn next_down(self) -> Self {
            Self(self.0.next_down())
        }
    }
}

impl From<f32> for Evaluation {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<Evaluation> for f32 {
    fn from(value: Evaluation) -> Self {
        value.0
    }
}

impl fmt::Debug for Evaluation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for Evaluation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Evaluation::MAX {
            "max".fmt(f)
        } else if *self == Evaluation::MIN {
            "min".fmt(f)
        } else if self.is_terminal() {
            if *self > 0.0.into() {
                "win".fmt(f)
            } else {
                "loss".fmt(f)
            }
        } else {
            write!(f, "{:.4}", self.0)
        }
    }
}
