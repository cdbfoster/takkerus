use serde::{Deserialize, Serialize};

use crate::gradient_descent::{Adam, GradientDescent, SimpleGradientDescent};
use crate::linear_algebra::{MatrixColumnMajor, Vector};

use super::ShallowAnn;

pub trait ShallowGradientDescent<const I: usize, const H: usize, const O: usize> {
    #[allow(clippy::too_many_arguments)]
    fn descend_shallow(
        &mut self,
        t: usize,
        rate: f32,
        ann: &mut ShallowAnn<I, H, O>,
        hidden_weight_gradients: &MatrixColumnMajor<I, H>,
        hidden_bias_gradients: &Vector<H>,
        output_weight_gradients: &MatrixColumnMajor<H, O>,
        output_bias_gradients: &Vector<O>,
    );
}

#[derive(Default, Deserialize, Serialize)]
pub struct ShallowAdam<const I: usize, const H: usize, const O: usize> {
    hidden: Adam<I, H>,
    output: Adam<H, O>,
}

impl<const I: usize, const H: usize, const O: usize> ShallowAdam<I, H, O> {
    pub fn new(beta1: f32, beta2: f32, epsilon: f32) -> Self {
        Self {
            hidden: Adam::new(beta1, beta2, epsilon),
            output: Adam::new(beta1, beta2, epsilon),
        }
    }
}

impl<const I: usize, const H: usize, const O: usize> ShallowGradientDescent<I, H, O>
    for ShallowAdam<I, H, O>
{
    fn descend_shallow(
        &mut self,
        t: usize,
        rate: f32,
        ann: &mut ShallowAnn<I, H, O>,
        hidden_weight_gradients: &MatrixColumnMajor<I, H>,
        hidden_bias_gradients: &Vector<H>,
        output_weight_gradients: &MatrixColumnMajor<H, O>,
        output_bias_gradients: &Vector<O>,
    ) {
        self.hidden.descend(
            t,
            rate,
            hidden_weight_gradients,
            hidden_bias_gradients,
            &mut ann.hidden_weights,
            &mut ann.hidden_biases,
        );

        self.output.descend(
            t,
            rate,
            output_weight_gradients,
            output_bias_gradients,
            &mut ann.output_weights,
            &mut ann.output_biases,
        );
    }
}

impl<const I: usize, const H: usize, const O: usize> ShallowGradientDescent<I, H, O>
    for SimpleGradientDescent
{
    fn descend_shallow(
        &mut self,
        t: usize,
        rate: f32,
        ann: &mut ShallowAnn<I, H, O>,
        hidden_weight_gradients: &MatrixColumnMajor<I, H>,
        hidden_bias_gradients: &Vector<H>,
        output_weight_gradients: &MatrixColumnMajor<H, O>,
        output_bias_gradients: &Vector<O>,
    ) {
        self.descend(
            t,
            rate,
            hidden_weight_gradients,
            hidden_bias_gradients,
            &mut ann.hidden_weights,
            &mut ann.hidden_biases,
        );

        self.descend(
            t,
            rate,
            output_weight_gradients,
            output_bias_gradients,
            &mut ann.output_weights,
            &mut ann.output_biases,
        );
    }
}
