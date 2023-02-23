use serde::{Deserialize, Serialize};

use crate::gradient_descent::{Adam, GradientDescent, SimpleGradientDescent};
use crate::linear_algebra::{MatrixColumnMajor, Vector};

use super::ShallowAnn;

pub trait ShallowGradientDescent<const I: usize, const H: usize, const O: usize> {
    #[allow(clippy::too_many_arguments)]
    fn descend_shallow(
        &mut self,
        t: usize,
        ann: &mut ShallowAnn<I, H, O>,
        hidden_weight_gradients: &MatrixColumnMajor<I, H>,
        hidden_bias_gradients: &Vector<H>,
        output_weight_gradients: &MatrixColumnMajor<H, O>,
        output_bias_gradients: &Vector<O>,
        rate: f32,
        l2_reg: f32,
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
        ann: &mut ShallowAnn<I, H, O>,
        hidden_weight_gradients: &MatrixColumnMajor<I, H>,
        hidden_bias_gradients: &Vector<H>,
        output_weight_gradients: &MatrixColumnMajor<H, O>,
        output_bias_gradients: &Vector<O>,
        rate: f32,
        l2_reg: f32,
    ) {
        self.hidden.descend(
            t,
            hidden_weight_gradients,
            hidden_bias_gradients,
            &mut ann.hidden_weights,
            &mut ann.hidden_biases,
            rate,
            l2_reg,
        );

        self.output.descend(
            t,
            output_weight_gradients,
            output_bias_gradients,
            &mut ann.output_weights,
            &mut ann.output_biases,
            rate,
            l2_reg,
        );
    }
}

impl<const I: usize, const H: usize, const O: usize> ShallowGradientDescent<I, H, O>
    for SimpleGradientDescent
{
    fn descend_shallow(
        &mut self,
        t: usize,
        ann: &mut ShallowAnn<I, H, O>,
        hidden_weight_gradients: &MatrixColumnMajor<I, H>,
        hidden_bias_gradients: &Vector<H>,
        output_weight_gradients: &MatrixColumnMajor<H, O>,
        output_bias_gradients: &Vector<O>,
        rate: f32,
        l2_reg: f32,
    ) {
        self.descend(
            t,
            hidden_weight_gradients,
            hidden_bias_gradients,
            &mut ann.hidden_weights,
            &mut ann.hidden_biases,
            rate,
            l2_reg,
        );

        self.descend(
            t,
            output_weight_gradients,
            output_bias_gradients,
            &mut ann.output_weights,
            &mut ann.output_biases,
            rate,
            l2_reg,
        );
    }
}
