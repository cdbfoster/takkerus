use crate::linear_algebra::{MatrixColumnMajor, MatrixRowMajor, Vector};

pub fn fully_connected_forward<const B: usize, const I: usize, const O: usize>(
    inputs: &MatrixRowMajor<B, I>,
    weights: &MatrixColumnMajor<I, O>,
    biases: &Vector<O>,
) -> MatrixRowMajor<B, O> {
    inputs * weights + biases
}

pub fn fully_connected_backward<const B: usize, const I: usize, const O: usize>(
    inputs: &MatrixRowMajor<B, I>,
    output_gradients: &MatrixRowMajor<B, O>,
    weights: &MatrixColumnMajor<I, O>,
) -> (MatrixColumnMajor<I, O>, Vector<O>, MatrixRowMajor<B, I>) {
    let weight_gradients = inputs.transpose().to_row_major() * output_gradients.to_column_major();
    let weight_gradients = weight_gradients.to_column_major();

    let bias_gradients = output_gradients
        .iter()
        .fold(Vector::zeros(), |sum, row| sum + row);

    let input_gradients = output_gradients * weights.transpose().to_column_major();

    (weight_gradients, bias_gradients, input_gradients)
}

pub fn activation_forward<const B: usize, const I: usize>(
    inputs: &MatrixRowMajor<B, I>,
    activation: impl Fn(f32) -> f32,
) -> MatrixRowMajor<B, I> {
    let mut outputs = *inputs;
    outputs.values_mut().for_each(|x| *x = activation(*x));
    outputs
}

pub fn activation_backward<const B: usize, const I: usize>(
    inputs: &MatrixRowMajor<B, I>,
    output_gradients: &MatrixRowMajor<B, I>,
    activation_prime: impl Fn(f32) -> f32,
) -> MatrixRowMajor<B, I> {
    let mut input_gradients = *output_gradients;
    input_gradients
        .values_mut()
        .zip(inputs.values())
        .for_each(|(x, &o)| *x *= activation_prime(o));
    input_gradients
}
