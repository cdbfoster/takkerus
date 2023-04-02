use crate::array::Array2;

/// Returns the coefficients of each input feature and the intercept.
pub(crate) fn lasso_regression(
    x: &Array2<f32>,
    y: &[f32],
    sample_weights: &[f32],
    alpha: f32,
    iterations: u32,
) -> (Vec<f32>, f32) {
    assert!(!x.is_empty());
    assert_eq!(x.len(), y.len());
    assert!((0.0..=1.0).contains(&alpha));
    assert!(iterations > 0);

    let [samples, features] = x.dim;

    // Scale sample weights such that they sum to `samples`.
    let mut sample_weights = sample_weights.to_vec();
    let sum = sample_weights.iter().sum::<f32>();
    sample_weights
        .iter_mut()
        .for_each(|sw| *sw *= samples as f32 / sum);

    // Calculate the mean for each feature.
    let x_mean = (0..features)
        .map(|i| {
            x.iter()
                .zip(&sample_weights)
                .map(|(x, sw)| x[i] * sw)
                .sum::<f32>()
                / samples as f32
        })
        .collect::<Vec<f32>>();

    // Center X.
    let mut x = x.clone();
    x.iter_mut()
        .flat_map(|x| x.iter_mut().zip(&x_mean))
        .for_each(|(x, mean)| *x -= mean);

    // Calculate the mean for the targets.
    let y_mean = y
        .iter()
        .zip(&sample_weights)
        .map(|(y, sw)| y * sw)
        .sum::<f32>()
        / samples as f32;

    // Center Y.
    let mut y = y.iter().map(|y| y - y_mean).collect::<Vec<_>>();

    // Apply sample weights.
    sample_weights
        .iter()
        .map(|sw| sw.sqrt())
        .zip(x.iter_mut().zip(&mut y))
        .for_each(|(sw, (x, y))| {
            x.iter_mut().for_each(|x| *x *= sw);
            *y *= sw;
        });

    // Square inputs and sum feature-wise.
    let x_norm = (0..features)
        .map(|i| x.iter().map(|x| x[i] * x[i]).sum())
        .collect::<Vec<f32>>();

    let mut w = vec![0.0f32; features];
    let mut r = y.clone(); // r = y - x.dot(w), but w is zero to begin with.

    for _ in 0..iterations {
        for feature in 0..features {
            if x_norm[feature] == 0.0 {
                continue;
            }

            if w[feature] != 0.0 {
                r.iter_mut()
                    .zip(x.iter())
                    .for_each(|(r, x)| *r += x[feature] * w[feature]);
            }

            let num = r
                .iter()
                .zip(x.iter())
                .map(|(r, x)| r * x[feature])
                .sum::<f32>();

            w[feature] =
                num.signum() * (num.abs() - alpha * samples as f32).max(0.0) / x_norm[feature];

            if w[feature] != 0.0 {
                r.iter_mut()
                    .zip(x.iter())
                    .for_each(|(r, x)| *r -= x[feature] * w[feature]);
            }
        }
    }

    let b = y_mean - x_mean.iter().zip(&w).map(|(x, w)| x * w).sum::<f32>();

    (w, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let x = Array2::from_vec(vec![1.0, 2.0, 3.0, 4.0], 1);
        let y = vec![1.0, 2.0, 3.0, 4.0];
        let w = vec![1.0; x.len()];

        let (w, b) = lasso_regression(&x, &y, &w, 0.0, 1);

        assert_eq!(w[0], 1.0);
        assert_eq!(b, 0.0);
    }
}
