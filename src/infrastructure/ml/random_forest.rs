#![allow(dead_code)]
use anyhow::{Result, anyhow};
use ndarray::{Array1, Array2};
use smartcore::ensemble::random_forest_classifier::{RandomForestClassifier, RandomForestClassifierParameters};
use smartcore::linalg::basic::matrix::DenseMatrix;

use crate::domain::traits::MLModel;
use crate::domain::models::RfHyperparams;

pub struct RandomForestModel {
    params: RfHyperparams,
    model: Option<RandomForestClassifier<f64, usize, DenseMatrix<f64>, Vec<usize>>>,
}

impl RandomForestModel {
    pub fn new(params: RfHyperparams) -> Self {
        Self {
            params,
            model: None,
        }
    }
}

impl MLModel for RandomForestModel {
    fn train(&mut self, features: &Array2<f64>, labels: &Array1<usize>) -> Result<()> {
        let x: Vec<f64> = features.iter().copied().collect();
        let x = DenseMatrix::new(features.nrows(), features.ncols(), x, false);
        let y: Vec<usize> = labels.to_vec();

        let mut params = RandomForestClassifierParameters::default();
        params = params.with_n_trees(self.params.n_trees as u16);
        params = params.with_min_samples_split(self.params.min_samples_split as usize);
        
        if let Some(depth) = self.params.max_depth {
            params = params.with_max_depth(depth as u16);
        }

        let model = RandomForestClassifier::fit(&x, &y, params)
            .map_err(|e| anyhow!("RF training failed: {:?}", e))?;
            
        self.model = Some(model);
        Ok(())
    }

    fn predict(&self, features: &Array2<f64>) -> Result<Vec<usize>> {
        let model = self.model.as_ref().ok_or_else(|| anyhow!("Model not trained"))?;
        
        let x: Vec<f64> = features.iter().copied().collect();
        let x = DenseMatrix::new(features.nrows(), features.ncols(), x, false);
        
        let predictions = model.predict(&x)
            .map_err(|e| anyhow!("RF predict failed: {:?}", e))?;
            
        Ok(predictions)
    }

    fn predict_proba(&self, _features: &Array2<f64>) -> Result<Vec<Vec<f64>>> {
        Err(anyhow!("predict_proba not fully supported in standard setup easily"))
    }

    fn save(&self) -> Result<Vec<u8>> {
        // Serialization placeholder
        Ok(vec![])
    }

    fn load(_data: &[u8]) -> Result<Self> where Self: Sized {
        Err(anyhow!("Load not implemented"))
    }
}
