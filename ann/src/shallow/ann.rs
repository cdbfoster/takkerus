use rand::distributions::{Distribution, Uniform};
use rand::Rng;
use rand_distr::Normal;

use crate::activation::{relu, relu_prime, tanh, tanh_prime};
use crate::layer::{
    activation_backward, activation_forward, fully_connected_backward, fully_connected_forward,
};
use crate::linear_algebra::{MatrixColumnMajor, MatrixRowMajor, Vector};

use super::ShallowGradientDescent;

#[derive(Clone, Debug)]
pub struct ShallowAnn<const I: usize, const H: usize, const O: usize> {
    pub hidden_weights: MatrixColumnMajor<I, H>,
    pub hidden_biases: Vector<H>,
    pub output_weights: MatrixColumnMajor<H, O>,
    pub output_biases: Vector<O>,
}

impl<const I: usize, const H: usize, const O: usize> ShallowAnn<I, H, O> {
    pub fn random(rng: &mut impl Rng) -> Self {
        // He initialization of the hidden weights.
        let normal_distribution = Normal::new(0.0, (2.0 / I as f32).sqrt()).unwrap();

        let mut hidden_weights = MatrixColumnMajor::zeros();
        hidden_weights
            .values_mut()
            .for_each(|x| *x = normal_distribution.sample(rng));

        // Glorot initialization of the output weights.
        let range = 6.0f32.sqrt() / (H as f32 + O as f32).sqrt();
        let uniform_distribution = Uniform::new(-range, range);

        let mut output_weights = MatrixColumnMajor::zeros();
        output_weights
            .values_mut()
            .for_each(|x| *x = uniform_distribution.sample(rng));

        Self {
            hidden_weights,
            hidden_biases: Vector::zeros(),
            output_weights,
            output_biases: Vector::zeros(),
        }
    }

    pub fn propagate_forward<const B: usize>(
        &self,
        inputs: &MatrixRowMajor<B, I>,
    ) -> MatrixRowMajor<B, O> {
        let mut hidden_layer = inputs * self.hidden_weights + self.hidden_biases;
        hidden_layer.values_mut().for_each(|x| *x = relu(*x));

        let mut output_layer = hidden_layer * self.output_weights + self.output_biases;
        output_layer.values_mut().for_each(|x| *x = tanh(*x));

        output_layer
    }

    pub fn train_batch<const B: usize>(
        &mut self,
        t: usize,
        rate: f32,
        inputs: &MatrixRowMajor<B, I>,
        labels: &MatrixRowMajor<B, O>,
        loss_prime: impl Fn(&MatrixRowMajor<B, O>, &MatrixRowMajor<B, O>) -> MatrixRowMajor<B, O>,
        gradient_descent: &mut impl ShallowGradientDescent<I, H, O>,
    ) {
        // Propagate forward ====================

        let hidden_fully_connected =
            fully_connected_forward(inputs, &self.hidden_weights, &self.hidden_biases);

        let hidden_activation = activation_forward(&hidden_fully_connected, relu);

        let output_fully_connected = fully_connected_forward(
            &hidden_activation,
            &self.output_weights,
            &self.output_biases,
        );

        let output_activation = activation_forward(&output_fully_connected, tanh);

        // Propagate backward ===================

        let output_activation_gradients = loss_prime(&output_activation, labels);

        let output_fully_connected_gradients = activation_backward(
            &output_fully_connected,
            &output_activation_gradients,
            tanh_prime,
        );

        let (output_weight_gradients, output_bias_gradients, hidden_activation_gradients) =
            fully_connected_backward(
                &hidden_activation,
                &output_fully_connected_gradients,
                &self.output_weights,
            );

        let hidden_fully_connected_gradients = activation_backward(
            &hidden_fully_connected,
            &hidden_activation_gradients,
            relu_prime,
        );

        let (hidden_weight_gradients, hidden_bias_gradients, _input_gradients) =
            fully_connected_backward(
                inputs,
                &hidden_fully_connected_gradients,
                &self.hidden_weights,
            );

        // Descend gradients ====================

        gradient_descent.descend_shallow(
            t,
            rate,
            self,
            &hidden_weight_gradients,
            &hidden_bias_gradients,
            &output_weight_gradients,
            &output_bias_gradients,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::loss::{mse, mse_prime};
    use crate::shallow::ShallowAdam;

    #[test]
    fn xor_net() {
        let inputs: MatrixRowMajor<4, 2> = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]].into();
        let labels: MatrixRowMajor<4, 1> = [[0.0], [1.0], [1.0], [0.0]].into();

        let mut ann = ShallowAnn::<2, 10, 1>::random(&mut rand::thread_rng());
        let mut adam = ShallowAdam::default();

        for t in 1..=1000 {
            ann.train_batch(t, 0.01, &inputs, &labels, mse_prime, &mut adam);
        }

        let outputs = ann.propagate_forward(&inputs);
        let error = mse(&outputs, &labels);

        assert!(error[0] < 0.001);
    }
}
