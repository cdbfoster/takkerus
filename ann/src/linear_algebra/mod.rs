pub use self::matrix::{MatrixColumnMajor, MatrixRowMajor};
pub use self::vector::Vector;

mod matrix;
mod vector;

pub type Value = f32;

pub trait ValueType {
    const ZERO: Self;
    const ONE: Self;
}

impl ValueType for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
}
