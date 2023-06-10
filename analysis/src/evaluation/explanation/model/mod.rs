use lime::helpers::{sample_features, vectorize_sample};
use lime::{Explainer, Sample};
use tak::State;

use crate::evaluation::{AnnEvaluator, AnnModel, GatherFeatures};

mod features_3s;
mod features_4s;
mod features_5s;
mod features_6s;
mod features_7s;
mod features_8s;

pub use self::model_3s_explainer::Model3sExplainer;
pub use self::model_4s_explainer::Model4sExplainer;
pub use self::model_5s_explainer::Model5sExplainer;
pub use self::model_6s_explainer::Model6sExplainer;
pub use self::model_7s_explainer::Model7sExplainer;
pub use self::model_8s_explainer::Model8sExplainer;

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

                    let perturbed_output =
                        self.model.propagate_forward(&perturbed_input.into())[0][0];

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
    };
}

model_explainer_impl!(
    size: 3,
    struct_name: Model3sExplainer,
    module: model_3s_explainer,
    features: features_3s
);

model_explainer_impl!(
    size: 4,
    struct_name: Model4sExplainer,
    module: model_4s_explainer,
    features: features_4s
);

model_explainer_impl!(
    size: 5,
    struct_name: Model5sExplainer,
    module: model_5s_explainer,
    features: features_5s
);

model_explainer_impl!(
    size: 6,
    struct_name: Model6sExplainer,
    module: model_6s_explainer,
    features: features_6s
);

model_explainer_impl!(
    size: 7,
    struct_name: Model7sExplainer,
    module: model_7s_explainer,
    features: features_7s
);

model_explainer_impl!(
    size: 8,
    struct_name: Model8sExplainer,
    module: model_8s_explainer,
    features: features_8s
);
