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
            "White to move",
            "Flat count differential",
            "Player: Reserve flatstones",
            "Player: Reserve capstones",
            "Player: Friendlies under flatstones",
            "Player: Friendlies under standing stones",
            "Player: Friendlies under capstones",
            "Player: Captives under flatstones",
            "Player: Captives under standing stones",
            "Player: Captives under capstones",
            "Player: Flatstones in shell 1",
            "Player: Flatstones in shell 2",
            "Player: Flatstones in shell 3",
            "Player: Flatstones in shell 4",
            "Player: Flatstones in shell 5",
            "Player: Capstones in shell 1",
            "Player: Capstones in shell 2",
            "Player: Capstones in shell 3",
            "Player: Capstones in shell 4",
            "Player: Capstones in shell 5",
            "Player: Road groups",
            "Player: Lines occupied",
            "Player: Critical squares",
            "Player: Enemy flatstones next to our standing stones",
            "Player: Enemy flatstones next to our capstones",
            "Opponent: Reserve flatstones",
            "Opponent: Reserve capstones",
            "Opponent: Friendlies under flatstones",
            "Opponent: Friendlies under standing stones",
            "Opponent: Friendlies under capstones",
            "Opponent: Captives under flatstones",
            "Opponent: Captives under standing stones",
            "Opponent: Captives under capstones",
            "Opponent: Flatstones in shell 1",
            "Opponent: Flatstones in shell 2",
            "Opponent: Flatstones in shell 3",
            "Opponent: Flatstones in shell 4",
            "Opponent: Flatstones in shell 5",
            "Opponent: Capstones in shell 1",
            "Opponent: Capstones in shell 2",
            "Opponent: Capstones in shell 3",
            "Opponent: Capstones in shell 4",
            "Opponent: Capstones in shell 5",
            "Opponent: Road groups",
            "Opponent: Lines occupied",
            "Opponent: Critical squares",
            "Opponent: Enemy flatstones next to our standing stones",
            "Opponent: Enemy flatstones next to our capstones",
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
