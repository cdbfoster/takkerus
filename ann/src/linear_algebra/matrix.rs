use std::fmt;
use std::ops::{
    Add, AddAssign, Deref, DerefMut, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign,
};

use super::{Value, ValueType, Vector};

macro_rules! matrix_impl {
    ($matrix:ident, $major:ident, $minor:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq)]
        pub struct $matrix<const R: usize, const C: usize>([Vector<$minor>; $major]);

        impl<const R: usize, const C: usize> $matrix<R, C> {
            pub const fn zeros() -> Self {
                Self([Vector::zeros(); $major])
            }

            pub const fn ones() -> Self {
                Self([Vector::ones(); $major])
            }

            pub fn new(values: [Vector<$minor>; $major]) -> Self {
                Self(values)
            }

            pub fn values(&self) -> impl Iterator<Item = &Value> {
                self.iter().flatten()
            }

            pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Value> {
                self.iter_mut().flatten()
            }
        }

        macro_rules! value_op_impl {
            ($op:ident, $op_method:ident, $op_assign:ident, $op_assign_method:ident) => {
                impl<const R: usize, const C: usize> $op<Value> for $matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(mut self, rhs: Value) -> Self::Output {
                        self.$op_assign_method(rhs);
                        self
                    }
                }

                impl<const R: usize, const C: usize> $op<Value> for &$matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(self, rhs: Value) -> Self::Output {
                        (*self).$op_method(rhs)
                    }
                }

                impl<const R: usize, const C: usize> $op<&Value> for $matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(mut self, rhs: &Value) -> Self::Output {
                        self.$op_assign_method(rhs);
                        self
                    }
                }

                impl<const R: usize, const C: usize> $op<&Value> for &$matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(self, rhs: &Value) -> Self::Output {
                        (*self).$op_method(rhs)
                    }
                }

                impl<const R: usize, const C: usize> $op_assign<Value> for $matrix<R, C> {
                    fn $op_assign_method(&mut self, rhs: Value) {
                        for r in self.iter_mut() {
                            (*r).$op_assign_method(rhs)
                        }
                    }
                }

                impl<const R: usize, const C: usize> $op_assign<&Value> for $matrix<R, C> {
                    fn $op_assign_method(&mut self, rhs: &Value) {
                        for r in self.iter_mut() {
                            (*r).$op_assign_method(rhs)
                        }
                    }
                }
            };
        }

        value_op_impl!(Add, add, AddAssign, add_assign);
        value_op_impl!(Sub, sub, SubAssign, sub_assign);
        value_op_impl!(Mul, mul, MulAssign, mul_assign);
        value_op_impl!(Div, div, DivAssign, div_assign);

        macro_rules! vector_op_impl {
            ($op:ident, $op_method:ident, $op_assign:ident, $op_assign_method:ident) => {
                impl<const R: usize, const C: usize> $op<Vector<$minor>> for $matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(mut self, rhs: Vector<$minor>) -> Self::Output {
                        self.$op_assign_method(rhs);
                        self
                    }
                }

                impl<const R: usize, const C: usize> $op<Vector<$minor>> for &$matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(self, rhs: Vector<$minor>) -> Self::Output {
                        (*self).$op_method(rhs)
                    }
                }

                impl<const R: usize, const C: usize> $op<&Vector<$minor>> for $matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(mut self, rhs: &Vector<$minor>) -> Self::Output {
                        self.$op_assign_method(rhs);
                        self
                    }
                }

                impl<const R: usize, const C: usize> $op<&Vector<$minor>> for &$matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(self, rhs: &Vector<$minor>) -> Self::Output {
                        (*self).$op_method(rhs)
                    }
                }

                impl<const R: usize, const C: usize> $op_assign<Vector<$minor>> for $matrix<R, C> {
                    fn $op_assign_method(&mut self, rhs: Vector<$minor>) {
                        for r in self.iter_mut() {
                            (*r).$op_assign_method(rhs)
                        }
                    }
                }

                impl<const R: usize, const C: usize> $op_assign<&Vector<$minor>> for $matrix<R, C> {
                    fn $op_assign_method(&mut self, rhs: &Vector<$minor>) {
                        for r in self.iter_mut() {
                            (*r).$op_assign_method(rhs)
                        }
                    }
                }
            };
        }

        vector_op_impl!(Add, add, AddAssign, add_assign);
        vector_op_impl!(Sub, sub, SubAssign, sub_assign);
        vector_op_impl!(Div, div, DivAssign, div_assign);

        macro_rules! matrix_op_impl {
            ($op:ident, $op_method:ident, $op_assign:ident, $op_assign_method:ident) => {
                impl<const R: usize, const C: usize> $op<$matrix<R, C>> for $matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(mut self, rhs: $matrix<R, C>) -> Self::Output {
                        self.$op_assign_method(rhs);
                        self
                    }
                }

                impl<const R: usize, const C: usize> $op<$matrix<R, C>> for &$matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(self, rhs: $matrix<R, C>) -> Self::Output {
                        (*self).$op_method(rhs)
                    }
                }

                impl<const R: usize, const C: usize> $op<&$matrix<R, C>> for $matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(mut self, rhs: &$matrix<R, C>) -> Self::Output {
                        self.$op_assign_method(rhs);
                        self
                    }
                }

                impl<const R: usize, const C: usize> $op<&$matrix<R, C>> for &$matrix<R, C> {
                    type Output = $matrix<R, C>;

                    fn $op_method(self, rhs: &$matrix<R, C>) -> Self::Output {
                        (*self).$op_method(rhs)
                    }
                }

                impl<const R: usize, const C: usize> $op_assign<$matrix<R, C>> for $matrix<R, C> {
                    fn $op_assign_method(&mut self, rhs: $matrix<R, C>) {
                        for (r, b) in self.iter_mut().zip(*rhs) {
                            (*r).$op_assign_method(b)
                        }
                    }
                }

                impl<const R: usize, const C: usize> $op_assign<&$matrix<R, C>> for $matrix<R, C> {
                    fn $op_assign_method(&mut self, rhs: &$matrix<R, C>) {
                        for (r, b) in self.iter_mut().zip(**rhs) {
                            (*r).$op_assign_method(b)
                        }
                    }
                }
            };
        }

        matrix_op_impl!(Add, add, AddAssign, add_assign);
        matrix_op_impl!(Sub, sub, SubAssign, sub_assign);
        matrix_op_impl!(Div, div, DivAssign, div_assign);

        impl<const R: usize, const C: usize> Neg for $matrix<R, C> {
            type Output = $matrix<R, C>;

            fn neg(self) -> Self::Output {
                self * -Value::ONE
            }
        }

        impl<const R: usize, const C: usize> Neg for &$matrix<R, C> {
            type Output = $matrix<R, C>;

            fn neg(self) -> Self::Output {
                self * -Value::ONE
            }
        }

        impl<const R: usize, const C: usize> Deref for $matrix<R, C> {
            type Target = [Vector<$minor>; $major];

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<const R: usize, const C: usize> DerefMut for $matrix<R, C> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl<const R: usize, const C: usize> From<[[Value; $minor]; $major]> for $matrix<R, C> {
            fn from(values: [[Value; $minor]; $major]) -> Self {
                Self(values.map(Vector::new))
            }
        }
    };
}

matrix_impl!(MatrixRowMajor, R, C);
matrix_impl!(MatrixColumnMajor, C, R);

impl<const R: usize, const C: usize> MatrixRowMajor<R, C> {
    pub fn to_column_major(self) -> MatrixColumnMajor<R, C> {
        let mut result = MatrixColumnMajor::zeros();
        for row in 0..R {
            for column in 0..C {
                result[column][row] = self[row][column];
            }
        }
        result
    }

    pub fn transpose(self) -> MatrixColumnMajor<C, R> {
        MatrixColumnMajor::new(self.0)
    }
}

impl<const R: usize, const C: usize> MatrixColumnMajor<R, C> {
    pub fn to_row_major(self) -> MatrixRowMajor<R, C> {
        let mut result = MatrixRowMajor::zeros();
        for column in 0..C {
            for row in 0..R {
                result[row][column] = self[column][row];
            }
        }
        result
    }

    pub fn transpose(self) -> MatrixRowMajor<C, R> {
        MatrixRowMajor::new(self.0)
    }
}

impl<const R: usize, const C: usize, const S: usize> Mul<MatrixColumnMajor<C, S>>
    for MatrixRowMajor<R, C>
{
    type Output = MatrixRowMajor<R, S>;

    fn mul(self, rhs: MatrixColumnMajor<C, S>) -> Self::Output {
        &self * &rhs
    }
}

impl<const R: usize, const C: usize, const S: usize> Mul<&MatrixColumnMajor<C, S>>
    for MatrixRowMajor<R, C>
{
    type Output = MatrixRowMajor<R, S>;

    #[allow(clippy::op_ref)]
    fn mul(self, rhs: &MatrixColumnMajor<C, S>) -> Self::Output {
        &self * rhs
    }
}

impl<const R: usize, const C: usize, const S: usize> Mul<MatrixColumnMajor<C, S>>
    for &MatrixRowMajor<R, C>
{
    type Output = MatrixRowMajor<R, S>;

    #[allow(clippy::op_ref)]
    fn mul(self, rhs: MatrixColumnMajor<C, S>) -> Self::Output {
        self * &rhs
    }
}

impl<const R: usize, const C: usize, const S: usize> Mul<&MatrixColumnMajor<C, S>>
    for &MatrixRowMajor<R, C>
{
    type Output = MatrixRowMajor<R, S>;

    fn mul(self, rhs: &MatrixColumnMajor<C, S>) -> Self::Output {
        let mut result = Self::Output::zeros();
        for row in 0..R {
            for column in 0..S {
                result[row][column] = self[row].dot(&rhs[column]);
            }
        }
        result
    }
}

impl<const C: usize> From<Vector<C>> for MatrixRowMajor<1, C> {
    fn from(values: Vector<C>) -> Self {
        Self([values])
    }
}

impl<const R: usize> From<Vector<R>> for MatrixColumnMajor<R, 1> {
    fn from(values: Vector<R>) -> Self {
        Self([values])
    }
}

impl<const R: usize, const C: usize> fmt::Debug for MatrixRowMajor<R, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in 0..R {
            write!(f, "{}", if row == 0 { "[" } else { " " })?;
            for column in 0..C {
                self[row][column].fmt(f)?;
                if column < C - 1 {
                    write!(f, " ")?;
                }
            }
            write!(f, "{}", if row < R - 1 { "\n" } else { "]" })?;
        }
        Ok(())
    }
}

impl<const R: usize, const C: usize> fmt::Debug for MatrixColumnMajor<R, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in 0..R {
            write!(f, "{}", if row == 0 { "[" } else { " " })?;
            for column in 0..C {
                self[column][row].fmt(f)?;
                if column < C - 1 {
                    write!(f, " ")?;
                }
            }
            write!(f, "{}", if row < R - 1 { "\n" } else { "]" })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cast() {
        let m: MatrixRowMajor<3, 3> = [[1.0, 2.0, 3.0], [4.0, -5.0, 6.0], [7.0, 8.0, 9.0]].into();
        let n: MatrixColumnMajor<3, 3> =
            [[1.0, 4.0, 7.0], [2.0, -5.0, 8.0], [3.0, 6.0, 9.0]].into();
        assert_eq!(m.to_column_major(), n);
        assert_eq!(n.to_row_major(), m);

        let m: MatrixRowMajor<2, 4> = [[1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0]].into();
        let n: MatrixColumnMajor<2, 4> = [[1.0, 5.0], [2.0, 6.0], [3.0, 7.0], [4.0, 8.0]].into();
        assert_eq!(m.to_column_major(), n);
        assert_eq!(n.to_row_major(), m);
    }

    #[test]
    fn transpose() {
        let m: MatrixRowMajor<3, 3> = [[1.0, 2.0, 3.0], [4.0, -5.0, 6.0], [7.0, 8.0, 9.0]].into();
        let n: MatrixColumnMajor<3, 3> =
            [[1.0, 2.0, 3.0], [4.0, -5.0, 6.0], [7.0, 8.0, 9.0]].into();
        assert_eq!(m.transpose(), n);

        let m: MatrixRowMajor<2, 4> = [[1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0]].into();
        let n: MatrixColumnMajor<4, 2> = [[1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0]].into();
        assert_eq!(m.transpose(), n);
    }

    #[test]
    fn multiply() {
        let m: MatrixRowMajor<2, 4> = [[1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0]].into();
        let n: MatrixColumnMajor<4, 3> = [
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
        ]
        .into();
        let o: MatrixRowMajor<2, 3> = [[30.0, 70.0, 110.0], [70.0, 174.0, 278.0]].into();
        assert_eq!(m * n, o);
    }
}
