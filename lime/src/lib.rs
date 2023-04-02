use std::fmt::Debug;

mod array;
mod lasso;

use self::array::Array2;
use self::lasso::lasso_regression;

pub struct Sample {
    pub features: Vec<f32>,
    pub label: f32,
    pub weight: f32,
}

#[derive(Debug)]
pub struct Explanation<F>
where
    F: Debug,
{
    pub evaluation: f32,
    pub feature_weights: Vec<FeatureWeight<F>>,
    pub intercept: f32,
}

#[derive(Debug)]
pub struct FeatureWeight<F>
where
    F: Debug,
{
    pub feature: F,
    pub weight: f32,
}

pub trait Explainer {
    type Input;
    type Feature: Debug;

    fn gather_features(&self, input: &Self::Input) -> Vec<Self::Feature>;

    fn baseline(&self, input: &Self::Input) -> Sample;

    fn sample_model(&self, input: &Self::Input, features: &[Self::Feature]) -> Sample;

    fn explain(&self, input: &Self::Input, samples: usize) -> Explanation<Self::Feature> {
        let baseline = self.baseline(input).label;

        let features = self.gather_features(input);

        let mut x = Vec::with_capacity(features.len() * samples);
        let mut y = Vec::with_capacity(samples);
        let mut sample_weights = Vec::with_capacity(samples);

        for _ in 0..samples {
            let sample = self.sample_model(input, &features);

            x.extend(sample.features);
            y.push(sample.label);
            sample_weights.push(sample.weight);
        }

        let x = Array2::from_vec(x, features.len());

        let (weights, intercept) = lasso_regression(&x, &y, &sample_weights, 0.001, 100);

        let feature_weights = features
            .into_iter()
            .zip(weights)
            .map(|(feature, weight)| FeatureWeight { feature, weight })
            .collect::<Vec<_>>();

        Explanation {
            evaluation: baseline,
            feature_weights,
            intercept,
        }
    }
}

pub mod helpers {
    use rand::{self, Rng};

    pub fn sample_features<F>(features: &[F]) -> Vec<bool> {
        let mut rng = rand::thread_rng();

        let mut values = vec![false; features.len()];
        rng.fill(values.as_mut_slice());

        values
    }

    pub fn vectorize_sample(sample: &[bool]) -> Vec<f32> {
        sample
            .iter()
            .map(|&feature| if feature { 1.0 } else { 0.0 })
            .collect()
    }
}
