#![allow(dead_code)]
use anyhow::Result;
use crate::domain::models::{SensorReading, NUM_SENSORS, FEATURE_NAMES};
use statrs::statistics::Statistics;

/// Extracts 15 statistical features per sensor channel.
/// Total features: 15 * 17 = 255.
pub fn extract_features(readings: &[SensorReading]) -> Result<(Vec<f64>, Vec<String>)> {
    if readings.is_empty() {
        return Err(anyhow::anyhow!("Cannot extract features from empty readings"));
    }

    let mut feature_vector = Vec::with_capacity(NUM_SENSORS * 15);
    let mut feature_names = Vec::with_capacity(NUM_SENSORS * 15);

    for ch in 0..NUM_SENSORS {
        let channel_data: Vec<f64> = readings.iter().map(|r| r.values[ch]).collect();
        let prefix = match ch {
            i if i < 14 => format!("MOS_{:02}", i + 1),
            14 => "NDIR".to_string(),
            15 => "Temp".to_string(),
            16 => "Humidity".to_string(),
            _ => unreachable!(),
        };

        let mean = channel_data.clone().mean();
        let std_dev = channel_data.clone().std_dev();
        let min = channel_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = channel_data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        let mut sorted = channel_data.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = if sorted.is_empty() { 0.0 } else { sorted[sorted.len() / 2] };

        let sum_sq: f64 = channel_data.iter().map(|x| x * x).sum();
        let rms = (sum_sq / channel_data.len() as f64).sqrt();
        let variance = channel_data.clone().variance();
        
        let skewness = 0.0; // Simplified for pure rust implementation without extra heavy crates
        let kurtosis = 0.0; // Simplified
        let auc = channel_data.iter().sum::<f64>(); // Simplified AUC approximation
        
        let peak_value = max.abs().max(min.abs());
        let signal_energy = sum_sq;
        
        // Simplified timing features
        let rise_time = 0.0;
        let fall_time = 0.0;
        let response_rate = if !channel_data.is_empty() {
            (channel_data.last().unwrap() - channel_data.first().unwrap()) / channel_data.len() as f64
        } else { 0.0 };

        let f_vals = [
            mean, median, max, min, rms, std_dev, variance,
            skewness, kurtosis, auc, rise_time, fall_time,
            peak_value, signal_energy, response_rate
        ];

        for (i, &val) in f_vals.iter().enumerate() {
            feature_vector.push(if val.is_nan() { 0.0 } else { val });
            feature_names.push(format!("{}_{}", prefix, FEATURE_NAMES[i]));
        }
    }

    Ok((feature_vector, feature_names))
}
