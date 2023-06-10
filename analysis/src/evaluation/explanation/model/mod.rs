use lime::helpers::{sample_features, vectorize_sample};
use lime::{Explainer, Sample};
use tak::State;

use crate::evaluation::{AnnEvaluator, AnnModel, GatherFeatures};

mod features_6s;

pub use self::model_6s_explainer::Model6sExplainer;

macro_rules! model_explainer_impl {
    (size: $size:expr, struct_name: $struct_name:ident, module: $module:ident, features: $features:ident) => {
        mod $module {
            use super::*;

            pub struct $struct_name {
                pub model: <AnnModel<$size> as AnnEvaluator<$size>>::Model,
            }

            impl Explainer for $struct_name {
                type Input = <State<$size> as GatherFeatures>::Features;
                type Feature = String;

                fn gather_features(&self, input: &Self::Input) -> Vec<Self::Feature> {
                    use $features::FEATURE_NAMES;

                    let input_vector = input.as_vector();

                    assert_eq!(input_vector.len(), FEATURE_NAMES.len());

                    FEATURE_NAMES
                        .iter()
                        .zip(input_vector)
                        .filter(|(_, &x)| x != 0.0)
                        .map(|(name, _)| (*name).to_owned())
                        .collect()
                }

                fn baseline(&self, input: &Self::Input) -> Sample {
                    Sample {
                        features: vec![1.0; input.as_vector().len()],
                        label: self.model.propagate_forward(input.as_vector().into())[0][0],
                        weight: 1.0,
                    }
                }

                fn sample_model(&self, input: &Self::Input, features: &[Self::Feature]) -> Sample {
                    let feature_mask = vectorize_sample(&sample_features(features));

                    let mut perturbed_input = input.as_vector().clone();
                    perturbed_input
                        .iter_mut()
                        .filter(|x| **x != 0.0)
                        .zip(&feature_mask)
                        .for_each(|(x, m)| *x *= m);

                    let perturbed_output = self.model.propagate_forward(&perturbed_input.into())[0][0];

                    let sum = feature_mask.iter().sum::<f32>();

                    // Cosine distance, given that the baseline is all ones and `feature_mask` is all ones and zeros.
                    let weight = if sum > 0.0 {
                        sum / (sum.sqrt() * (features.len() as f32).sqrt())
                    } else {
                        0.0
                    };

                    Sample {
                        features: feature_mask,
                        label: perturbed_output,
                        weight,
                    }
                }
            }
        }
    }
}

model_explainer_impl!(
    size: 6,
    struct_name: Model6sExplainer,
    module: model_6s_explainer,
    features: features_6s
);