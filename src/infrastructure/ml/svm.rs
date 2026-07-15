#![allow(dead_code)]
use anyhow::{Result, anyhow};
use ndarray::{Array1, Array2};

use crate::domain::traits::MLModel;
use crate::domain::models::SvmHyperparams;

pub struct SvmModel {
    #[allow(dead_code)]
    params: SvmHyperparams,
    is_trained: bool,
}

impl SvmModel {
    pub fn new(params: SvmHyperparams) -> Self {
        Self {
            params,
            is_trained: false,
        }
    }
}

impl MLModel for SvmModel {
    fn train(&mut self, _features: &Array2<f64>, _labels: &Array1<usize>) -> Result<()> {
        self.is_trained = true;
        Ok(())
    }

    fn predict(&self, features: &Array2<f64>) -> Result<Vec<usize>> {
        if !self.is_trained {
            return Err(anyhow!("Model not trained"));
        }
        Ok(vec![0; features.nrows()])
    }

    fn predict_proba(&self, _features: &Array2<f64>) -> Result<Vec<Vec<f64>>> {
        Err(anyhow!("predict_proba not implemented for SVM"))
    }

    fn save(&self) -> Result<Vec<u8>> {
        if self.is_trained {
            Ok(vec![])
        } else {
            Err(anyhow!("Model not trained"))
        }
    }

    fn load(_data: &[u8]) -> Result<Self> where Self: Sized {
        Err(anyhow!("Load not implemented"))
    }
}
