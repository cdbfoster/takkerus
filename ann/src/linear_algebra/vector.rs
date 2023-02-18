use std::fmt;
use std::ops::{
    Add, AddAssign, Deref, DerefMut, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign,
};

use serde::{Deserialize, Serialize};

use super::{array_serde, Value, ValueType};

#[repr(transparent)]
#[derive(Clone, Copy, Deserialize, PartialEq, Serialize)]
pub struct Vector<const N: usize>(#[serde(with = "array_serde")] [Value; N]);

impl<const N: usize> Vector<N> {
    pub const fn zeros() -> Self {
        Self([Value::ZERO; N])
    }

    pub const fn ones() -> Self {
        Self([Value::ONE; N])
    }

    pub const fn new(values: [Value; N]) -> Self {
        Self(values)
    }

    pub fn dot(&self, b: &Self) -> Value {
        self.iter().zip(b).map(|(a, b)| a * b).sum()
    }
}

macro_rules! op_impl {
    ($op:ident, $op_method:ident, $op_assign:ident, $op_assign_method:ident) => {
        impl<const N: usize> $op<Value> for Vector<N> {
            type Output = Vector<N>;

            fn $op_method(mut self, rhs: Value) -> Self::Output {
                self.$op_assign_method(rhs);
                self
            }
        }

        impl<const N: usize> $op<Value> for &Vector<N> {
            type Output = Vector<N>;

            fn $op_method(self, rhs: Value) -> Self::Output {
                (*self).$op_method(rhs)
            }
        }

        impl<const N: usize> $op<&Value> for Vector<N> {
            type Output = Vector<N>;

            fn $op_method(mut self, rhs: &Value) -> Self::Output {
                self.$op_assign_method(rhs);
                self
            }
        }

        impl<const N: usize> $op<&Value> for &Vector<N> {
            type Output = Vector<N>;

            fn $op_method(self, rhs: &Value) -> Self::Output {
                (*self).$op_method(rhs)
            }
        }

        impl<const N: usize> $op<Vector<N>> for Vector<N> {
            type Output = Vector<N>;

            fn $op_method(mut self, rhs: Vector<N>) -> Self::Output {
                self.$op_assign_method(rhs);
                self
            }
        }

        impl<const N: usize> $op<Vector<N>> for &Vector<N> {
            type Output = Vector<N>;

            fn $op_method(self, rhs: Vector<N>) -> Self::Output {
                (*self).$op_method(rhs)
            }
        }

        impl<const N: usize> $op<&Vector<N>> for Vector<N> {
            type Output = Vector<N>;

            fn $op_method(mut self, rhs: &Vector<N>) -> Self::Output {
                self.$op_assign_method(rhs);
                self
            }
        }

        impl<const N: usize> $op<&Vector<N>> for &Vector<N> {
            type Output = Vector<N>;

            fn $op_method(self, rhs: &Vector<N>) -> Self::Output {
                (*self).$op_method(rhs)
            }
        }

        impl<const N: usize> $op_assign<Value> for Vector<N> {
            fn $op_assign_method(&mut self, rhs: Value) {
                for r in self.iter_mut() {
                    (*r).$op_assign_method(rhs)
                }
            }
        }

        impl<const N: usize> $op_assign<&Value> for Vector<N> {
            fn $op_assign_method(&mut self, rhs: &Value) {
                for r in self.iter_mut() {
                    (*r).$op_assign_method(rhs)
                }
            }
        }

        impl<const N: usize> $op_assign<Vector<N>> for Vector<N> {
            fn $op_assign_method(&mut self, rhs: Vector<N>) {
                for (r, b) in self.iter_mut().zip(*rhs) {
                    (*r).$op_assign_method(b)
                }
            }
        }

        impl<const N: usize> $op_assign<&Vector<N>> for Vector<N> {
            fn $op_assign_method(&mut self, rhs: &Vector<N>) {
                for (r, b) in self.iter_mut().zip(**rhs) {
                    (*r).$op_assign_method(b)
                }
            }
        }
    };
}

op_impl!(Add, add, AddAssign, add_assign);
op_impl!(Sub, sub, SubAssign, sub_assign);
op_impl!(Mul, mul, MulAssign, mul_assign);
op_impl!(Div, div, DivAssign, div_assign);

impl<const N: usize> Neg for Vector<N> {
    type Output = Vector<N>;

    fn neg(self) -> Self::Output {
        self * -Value::ONE
    }
}

impl<const N: usize> Neg for &Vector<N> {
    type Output = Vector<N>;

    fn neg(self) -> Self::Output {
        self * -Value::ONE
    }
}

impl<const N: usize> Deref for Vector<N> {
    type Target = [Value; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> DerefMut for Vector<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const N: usize> From<[Value; N]> for Vector<N> {
    fn from(values: [Value; N]) -> Self {
        Self(values)
    }
}

impl<'a, const N: usize> IntoIterator for &'a Vector<N> {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, const N: usize> IntoIterator for &'a mut Vector<N> {
    type Item = &'a mut Value;
    type IntoIter = std::slice::IterMut<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<const N: usize> fmt::Debug for Vector<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for x in 0..N {
            self[x].fmt(f)?;
            if x < N - 1 {
                write!(f, " ")?;
            }
        }
        write!(f, "]")
    }
}
