//! Inferensi fault class menggunakan Random Forest yang diekspor ke JSON.
//! Input: 5 gas confidence (0–1 range) dari SNN.
//! Output: string kelas fault ("T1" | "D1" | "N").

use serde::Deserialize;
use std::path::Path;

const LEAF: i64 = -1;

#[derive(Deserialize)]
pub struct RfTree {
    pub children_left:  Vec<i64>,
    pub children_right: Vec<i64>,
    pub feature:        Vec<i64>,
    pub threshold:      Vec<f64>,
    /// value[node] = [count_class0, count_class1, ...] (n_outputs=1 sudah di-flatten)
    pub value:          Vec<Vec<f64>>,
}

#[derive(Deserialize)]
pub struct RfModel {
    pub trees:      Vec<RfTree>,
    pub classes:    Vec<String>,
    pub n_features: usize,
    pub gas_classes: Vec<String>,
}

impl RfModel {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let s = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&s)?)
    }
}

/// Prediksi satu pohon — traverse hingga daun, return index kelas terbanyak.
fn predict_tree(tree: &RfTree, x: &[f64]) -> usize {
    let mut node = 0usize;
    loop {
        let left = tree.children_left[node];
        let right = tree.children_right[node];
        let feat = tree.feature[node];
        if left == -1 || right == -1 || feat < 0 {
            // Daun: pilih kelas dengan count terbesar
            return tree.value[node].iter().enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b)
                        .unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
        if (feat as usize) < x.len() && x[feat as usize] <= tree.threshold[node] {
            node = left as usize;
        } else {
            node = right as usize;
        }
    }
}

/// Prediksi majority-vote dari semua pohon.
/// Input `gas_conf`: slice 5 elemen [C2H2, C2H4, CH4, H2, OTHER] dalam range 0–1.
pub fn predict<'a>(model: &'a RfModel, gas_conf: &[f64]) -> &'a str {
    let n = model.classes.len();
    let mut votes = vec![0usize; n];
    for tree in &model.trees {
        let idx = predict_tree(tree, gas_conf);
        if idx < n { votes[idx] += 1; }
    }
    let best = votes.iter().enumerate()
        .max_by_key(|(_, &v)| v)
        .map(|(i, _)| i)
        .unwrap_or(0);
    &model.classes[best]
}
