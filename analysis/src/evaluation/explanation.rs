use lime::helpers::{sample_features, vectorize_sample};
use lime::{Explainer, Sample};
use tak::State;

use crate::evaluation::{AnnEvaluator, AnnModel, GatherFeatures};

pub struct Model6sExplainer {
    pub model: <AnnModel<6> as AnnEvaluator<6>>::Model,
}

impl Explainer for Model6sExplainer {
    type Input = <State<6> as GatherFeatures>::Features;
    type Feature = String;

    fn gather_features(&self, input: &Self::Input) -> Vec<Self::Feature> {
        [
            "Ply count",
            "White to move",
            "Flat count differential",
            "Reserve flatstones",
            "Reserve capstones",
            "Flatstones at a1 symmetries",
            "Flatstones at b1 symmetries",
            "Flatstones at c1 symmetries",
            "Flatstones at b2 symmetries",
            "Flatstones at c2 symmetries",
            "Flatstones at c3 symmetries",
            "Standing stones at a1 symmetries",
            "Standing stones at b1 symmetries",
            "Standing stones at c1 symmetries",
            "Standing stones at b2 symmetries",
            "Standing stones at c2 symmetries",
            "Standing stones at c3 symmetries",
            "Capstones at a1 symmetries",
            "Capstones at b1 symmetries",
            "Capstones at c1 symmetries",
            "Capstones at b2 symmetries",
            "Capstones at c2 symmetries",
            "Capstones at c3 symmetries",
            "Flatstones next to friendly flatstones",
            "Flatstones next to friendly standing stones",
            "Flatstones next to friendly capstones",
            "Flatstones next to enemy flatstones",
            "Flatstones next to enemy standing stones",
            "Flatstones next to enemy capstones",
            "Standing stones next to friendly flatstones",
            "Standing stones next to friendly standing stones",
            "Standing stones next to friendly capstones",
            "Standing stones next to enemy flatstones",
            "Standing stones next to enemy standing stones",
            "Standing stones next to enemy capstones",
            "Capstones next to friendly flatstones",
            "Capstones next to friendly standing stones",
            "Capstones next to friendly capstones",
            "Capstones next to enemy flatstones",
            "Capstones next to enemy standing stones",
            "Capstones next to enemy capstones",
            "Captives under flatstones",
            "Captives under standing stones",
            "Captives under capstones",
            "Friendlies under flatstones",
            "Friendlies under standing stones",
            "Friendlies under capstones",
            "Lines occupied",
            "Road groups",
            "Critical squares",
        ]
        .into_iter()
        .zip(input.as_vector())
        .filter(|(_, &x)| x != 0.0)
        .map(|(name, _)| name.to_owned())
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
