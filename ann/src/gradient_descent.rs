use serde::{Deserialize, Serialize};

use crate::linear_algebra::{MatrixColumnMajor, Vector};

pub trait GradientDescent<const I: usize, const O: usize> {
    #[allow(clippy::too_many_arguments)]
    fn descend(
        &mut self,
        t: usize,
        weight_gradients: &MatrixColumnMajor<I, O>,
        bias_gradients: &Vector<O>,
        weights: &mut MatrixColumnMajor<I, O>,
        biases: &mut Vector<O>,
        rate: f32,
        l2_reg: f32,
    );
}

#[derive(Deserialize, Serialize)]
pub struct Adam<const I: usize, const O: usize> {
    beta1: f32,
    beta2: f32,
    epsilon: f32,
    weight_momentum: MatrixColumnMajor<I, O>,
    weight_rms: MatrixColumnMajor<I, O>,
    bias_momentum: Vector<O>,
    bias_rms: Vector<O>,
}

impl<const I: usize, const O: usize> Default for Adam<I, O> {
    fn default() -> Self {
        Self::new(0.9, 0.999, f32::EPSILON)
    }
}

impl<const I: usize, const O: usize> Adam<I, O> {
    pub fn new(beta1: f32, beta2: f32, epsilon: f32) -> Self {
        Self {
            beta1,
            beta2,
            epsilon,
            weight_momentum: MatrixColumnMajor::zeros(),
            weight_rms: MatrixColumnMajor::zeros(),
            bias_momentum: Vector::zeros(),
            bias_rms: Vector::zeros(),
        }
    }
}

impl<const I: usize, const O: usize> GradientDescent<I, O> for Adam<I, O> {
    fn descend(
        &mut self,
        t: usize,
        weight_gradients: &MatrixColumnMajor<I, O>,
        bias_gradients: &Vector<O>,
        weights: &mut MatrixColumnMajor<I, O>,
        biases: &mut Vector<O>,
        rate: f32,
        l2_reg: f32,
    ) {
        // L2 regularization ====================

        let weight_gradients = weight_gradients + *weights * l2_reg;

        // Momentum update ======================

        self.weight_momentum *= self.beta1;
        self.weight_momentum += weight_gradients * (1.0 - self.beta1);

        self.bias_momentum *= self.beta1;
        self.bias_momentum += bias_gradients * (1.0 - self.beta1);

        // RMS update ===========================

        let mut weight_gradients_squared = weight_gradients;
        weight_gradients_squared.values_mut().for_each(|x| *x *= *x);
        self.weight_rms *= self.beta2;
        self.weight_rms += weight_gradients_squared * (1.0 - self.beta2);

        let bias_gradients_squared = bias_gradients * bias_gradients;
        self.bias_rms *= self.beta2;
        self.bias_rms += bias_gradients_squared * (1.0 - self.beta2);

        // Correction for bias ==================

        let weight_momentum_c = self.weight_momentum / (1.0 - self.beta1.powi(t as i32));
        let bias_momentum_c = self.bias_momentum / (1.0 - self.beta1.powi(t as i32));
        let weight_rms_c = self.weight_rms / (1.0 - self.beta2.powi(t as i32));
        let bias_rms_c = self.bias_rms / (1.0 - self.beta2.powi(t as i32));

        // Descend gradients ====================

        let mut weight_rm = weight_rms_c;
        weight_rm.values_mut().for_each(|x| *x = x.sqrt());
        *weights -= (weight_momentum_c / (weight_rm + self.epsilon)) * rate;

        let mut bias_rm = bias_rms_c;
        bias_rm.iter_mut().for_each(|x| *x = x.sqrt());
        *biases -= (bias_momentum_c / (bias_rm + self.epsilon)) * rate;
    }
}

#[derive(Default, Deserialize, Serialize)]
pub struct SimpleGradientDescent;

impl<const I: usize, const O: usize> GradientDescent<I, O> for SimpleGradientDescent {
    fn descend(
        &mut self,
        _t: usize,
        weight_gradients: &MatrixColumnMajor<I, O>,
        bias_gradients: &Vector<O>,
        weights: &mut MatrixColumnMajor<I, O>,
        biases: &mut Vector<O>,
        rate: f32,
        l2_reg: f32,
    ) {
        *weights -= (weight_gradients + *weights * l2_reg) * rate;
        *biases -= bias_gradients * rate;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adam_find_minimum() {
        // Derivative of x^2 - 2x + 1
        fn f_prime(x: f32) -> f32 {
            2.0 * x - 2.0
        }

        let mut w = MatrixColumnMajor::<1, 1>::zeros();

        let mut adam = Adam::default();

        for t in 1..=500 {
            let mut gradient = w;
            gradient.values_mut().for_each(|x| *x = f_prime(*x));

            adam.descend(
                t,
                &gradient,
                &Vector::zeros(),
                &mut w,
                &mut Vector::zeros(),
                0.01,
                0.0,
            );
        }

        // Minimum is at 1.0.
        assert!((w[0][0] - 1.0).abs() < 0.00001);
    }

    #[test]
    fn simple_find_minimum() {
        // Derivative of x^2 - 2x + 1
        fn f_prime(x: f32) -> f32 {
            2.0 * x - 2.0
        }

        let mut w = MatrixColumnMajor::<1, 1>::zeros();

        let mut simple = SimpleGradientDescent::default();

        for t in 1..=500 {
            let mut gradient = w;
            gradient.values_mut().for_each(|x| *x = f_prime(*x));

            simple.descend(
                t,
                &gradient,
                &Vector::zeros(),
                &mut w,
                &mut Vector::zeros(),
                0.01,
                0.0,
            );
        }

        // Minimum is at 1.0.
        assert!((w[0][0] - 1.0).abs() < 0.0001);
    }
}
