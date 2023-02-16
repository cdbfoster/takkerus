use crate::linear_algebra::{MatrixRowMajor, Vector};

/// Calculates the Mean Squared Error.
pub fn mse<const B: usize, const O: usize>(
    outputs: &MatrixRowMajor<B, O>,
    labels: &MatrixRowMajor<B, O>,
) -> Vector<O> {
    let mut error = labels - outputs;
    error.iter_mut().for_each(|row| *row *= *row);

    (error / B as f32)
        .iter()
        .fold(Vector::zeros(), |sum, row| sum + row)
}

/// Calculates the derivative of the Mean Squared Error function.
pub fn mse_prime<const B: usize, const O: usize>(
    outputs: &MatrixRowMajor<B, O>,
    labels: &MatrixRowMajor<B, O>,
) -> MatrixRowMajor<B, O> {
    (outputs - labels) * 2.0 / B as f32
}
