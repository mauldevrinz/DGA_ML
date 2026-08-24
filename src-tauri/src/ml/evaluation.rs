// src/evaluation.rs - Evaluation Metrics

use colored::*;
use ndarray::Array1;

#[derive(Debug, Clone)]
pub struct ClassMetrics {
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub support: usize,
}

#[derive(Debug, Clone)]
pub struct EvaluationResults {
    pub accuracy: f64,
    pub class_metrics: [ClassMetrics; 4], // 0: Baseline, 1: Normal, 2: Overheating, 3: Arcing
    pub confusion_matrix: [[usize; 4]; 4],
}

impl EvaluationResults {
    pub fn macro_precision(&self) -> f32 {
        let sum: f64 = self.class_metrics.iter().map(|m| m.precision).sum();
        (sum / 4.0) as f32
    }
    pub fn macro_recall(&self) -> f32 {
        let sum: f64 = self.class_metrics.iter().map(|m| m.recall).sum();
        (sum / 4.0) as f32
    }
    pub fn macro_f1(&self) -> f32 {
        let sum: f64 = self.class_metrics.iter().map(|m| m.f1_score).sum();
        (sum / 4.0) as f32
    }
}

pub struct Evaluator;

impl Evaluator {
    pub fn evaluate(predictions: &Array1<i64>, labels: &Array1<i64>) -> EvaluationResults {
        assert_eq!(predictions.len(), labels.len(), "Predictions and labels must have same length");
        
        let mut confusion_matrix = [[0; 4]; 4];
        
        for (pred, true_label) in predictions.iter().zip(labels.iter()) {
            let pred_idx = *pred as usize;
            let true_idx = *true_label as usize;
            if true_idx < 4 && pred_idx < 4 {
                confusion_matrix[true_idx][pred_idx] += 1;
            }
        }
        
        let mut class_metrics = [ClassMetrics { precision: 0.0, recall: 0.0, f1_score: 0.0, support: 0 }; 4];
        let mut correct = 0;
        let mut total = 0;

        for i in 0..4 {
            let tp = confusion_matrix[i][i];
            let mut fn_count = 0;
            let mut fp = 0;
            
            for j in 0..4 {
                if i != j {
                    fn_count += confusion_matrix[i][j]; // Actual i, Predicted j
                    fp += confusion_matrix[j][i]; // Actual j, Predicted i
                }
            }
            
            let precision = if tp + fp > 0 { tp as f64 / (tp + fp) as f64 } else { 0.0 };
            let recall = if tp + fn_count > 0 { tp as f64 / (tp + fn_count) as f64 } else { 0.0 };
            let f1_score = if precision + recall > 0.0 { 2.0 * (precision * recall) / (precision + recall) } else { 0.0 };
            let support = tp + fn_count;
            
            class_metrics[i] = ClassMetrics { precision, recall, f1_score, support };
            correct += tp;
            total += support;
        }
        
        let accuracy = if total > 0 { correct as f64 / total as f64 } else { 0.0 };
        
        EvaluationResults {
            accuracy,
            class_metrics,
            confusion_matrix,
        }
    }
    
    pub fn print_results(results: &EvaluationResults, dataset_name: &str) {
        println!("\n{}", format!("═══════════════════════════════════════════════════").cyan().bold());
        println!("{}", format!("  {} Set Evaluation Results", dataset_name).cyan().bold());
        println!("{}", format!("═══════════════════════════════════════════════════").cyan().bold());
        
        println!("\n{}", "📊 Overall Metrics:".bold());
        println!("┌─────────────────────┬──────────────┐");
        println!("│ Metric              │ Value        │");
        println!("├─────────────────────┼──────────────┤");
        println!("│ Overall Accuracy    │ {:>11.2}% │", results.accuracy * 100.0);
        println!("└─────────────────────┴──────────────┘");
        
        println!("\n{}", "📈 Per-Class Metrics:".bold());
        println!("┌─────────────┬────────────┬────────────┬────────────┬────────────┐");
        println!("│ Metric      │ Baseline   │ Normal     │ Overheat   │ Arcing     │");
        println!("├─────────────┼────────────┼────────────┼────────────┼────────────┤");
        println!("│ Precision   │ {:>10.3} │ {:>10.3} │ {:>10.3} │ {:>10.3} │", 
            results.class_metrics[0].precision, results.class_metrics[1].precision, results.class_metrics[2].precision, results.class_metrics[3].precision
        );
        println!("│ Recall      │ {:>10.3} │ {:>10.3} │ {:>10.3} │ {:>10.3} │",
            results.class_metrics[0].recall, results.class_metrics[1].recall, results.class_metrics[2].recall, results.class_metrics[3].recall
        );
        println!("│ F1-Score    │ {:>10.3} │ {:>10.3} │ {:>10.3} │ {:>10.3} │",
            results.class_metrics[0].f1_score, results.class_metrics[1].f1_score, results.class_metrics[2].f1_score, results.class_metrics[3].f1_score
        );
        println!("│ Support     │ {:>10} │ {:>10} │ {:>10} │ {:>10} │",
            results.class_metrics[0].support, results.class_metrics[1].support, results.class_metrics[2].support, results.class_metrics[3].support
        );
        println!("└─────────────┴────────────┴────────────┴────────────┴────────────┘");
        
        println!("\n{}", "🎯 Confusion Matrix:".bold());
        println!("┌─────────────┬─────────┬─────────┬─────────┬─────────┐");
        println!("│ Actual\\Pred │ Base    │ Norm    │ Over    │ Arc     │");
        println!("├─────────────┼─────────┼─────────┼─────────┼─────────┤");
        for (i, name) in ["Base", "Norm", "Over", "Arc "].iter().enumerate() {
            println!("│ True: {}  │ {:>7} │ {:>7} │ {:>7} │ {:>7} │",
                name,
                results.confusion_matrix[i][0], results.confusion_matrix[i][1], results.confusion_matrix[i][2], results.confusion_matrix[i][3]
            );
        }
        println!("└─────────────┴─────────┴─────────┴─────────┴─────────┘");
        
        Self::print_performance_analysis(results);
    }
    
    fn print_performance_analysis(results: &EvaluationResults) {
        println!("\n{}", "💡 Performance Analysis:".bold());
        
        let mut messages = Vec::new();
        
        if results.accuracy < 0.7 {
            messages.push("⚠️  Low overall accuracy (<70%). Consider improving model.".yellow().to_string());
        } else if results.accuracy > 0.95 {
            messages.push("⚠️  Very high accuracy (>95%). Check for overfitting.".yellow().to_string());
        } else {
            messages.push("✅ Overall accuracy is in good range (70-95%)".green().to_string());
        }
        
        let min_recall = results.class_metrics.iter().map(|m| m.recall).fold(1.0f64, f64::min);
        if min_recall < 0.7 {
            messages.push("⚠️  Low recall in one or more classes. Misclassifications are frequent.".yellow().to_string());
        }
        
        for msg in messages {
            println!("  {}", msg);
        }
        
        println!("\n{}", "💡 Recommendations:".bold());
        if results.accuracy >= 0.85 {
            println!("  ✅ Model performance is good!");
            println!("  • Consider deploying to production");
            println!("  • Monitor performance on new data");
        } else if results.accuracy >= 0.70 {
            println!("  🔄 Model performance is acceptable but could improve:");
            println!("  • Try data augmentation");
            println!("  • Tune hyperparameters");
            println!("  • Collect more diverse samples");
        } else {
            println!("  ⚠️  Model needs significant improvement:");
            println!("  • Review data quality");
            println!("  • Try different model architecture");
            println!("  • Increase dataset size");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;
    
    #[test]
    fn test_perfect_prediction() {
        let predictions = array![0, 0, 1, 1, 0, 1];
        let labels = array![0, 0, 1, 1, 0, 1];
        
        let results = Evaluator::evaluate(&predictions, &labels);
        
        assert_eq!(results.accuracy, 1.0);
        assert_eq!(results.class_metrics[0].precision, 1.0);
        assert_eq!(results.class_metrics[1].recall, 1.0);
    }
}