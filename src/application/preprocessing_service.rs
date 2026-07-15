#![allow(dead_code)]
use anyhow::Result;
use ndarray::Array2;
use crate::domain::models::NormalizationParams;
use crate::domain::enums::NormalizationMethod;

/// Applies normalization to the feature matrix.
pub fn normalize_features(
    features: &mut Array2<f64>,
    method: NormalizationMethod,
) -> Result<NormalizationParams> {
    if features.is_empty() {
        return Err(anyhow::anyhow!("Cannot normalize empty feature matrix"));
    }

    let n_cols = features.ncols();
    let mut means = vec![0.0; n_cols];
    let mut stds = vec![0.0; n_cols];
    let mut mins = vec![0.0; n_cols];
    let mut maxs = vec![0.0; n_cols];

    match method {
        NormalizationMethod::ZScore => {
            for j in 0..n_cols {
                let col = features.column(j);
                let mean = col.mean().unwrap_or(0.0);
                let std = col.std(0.0);
                means[j] = mean;
                stds[j] = std;

                let mut col_mut = features.column_mut(j);
                if std > 0.0 {
                    col_mut.mapv_inplace(|v| (v - mean) / std);
                } else {
                    col_mut.mapv_inplace(|v| v - mean);
                }
            }
        }
        NormalizationMethod::MinMax => {
            for j in 0..n_cols {
                let col = features.column(j);
                let min = col.fold(f64::INFINITY, |a, &b| a.min(b));
                let max = col.fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                mins[j] = min;
                maxs[j] = max;

                let range = max - min;
                let mut col_mut = features.column_mut(j);
                if range > 0.0 {
                    col_mut.mapv_inplace(|v| (v - min) / range);
                } else {
                    col_mut.mapv_inplace(|v| v - min); // or just 0
                }
            }
        }
    }

    Ok(NormalizationParams {
        method: method.as_str().to_string(),
        means,
        stds,
        mins,
        maxs,
    })
}

/// Apply existing normalization parameters to a new single feature vector (for inference)
pub fn apply_normalization(
    feature_vector: &mut [f64],
    params: &NormalizationParams,
) {
    let n = feature_vector.len();
    if params.method == "Z-Score" && params.means.len() == n && params.stds.len() == n {
        for i in 0..n {
            if params.stds[i] > 0.0 {
                feature_vector[i] = (feature_vector[i] - params.means[i]) / params.stds[i];
            } else {
                feature_vector[i] -= params.means[i];
            }
        }
    } else if params.method == "Min-Max" && params.mins.len() == n && params.maxs.len() == n {
        for i in 0..n {
            let range = params.maxs[i] - params.mins[i];
            if range > 0.0 {
                feature_vector[i] = (feature_vector[i] - params.mins[i]) / range;
            } else {
                feature_vector[i] -= params.mins[i];
            }
        }
    }
}
