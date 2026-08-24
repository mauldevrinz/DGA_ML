use dga::ml::report::{generate_training_pdf, TrainingReport};
use dga::ml::evaluation::{EvaluationResults, ClassMetrics};
use dga::power_monitor::PowerSummary;

fn main() {
    let mock_eval = EvaluationResults {
        accuracy: 0.985,
        high_grade_metrics: ClassMetrics { precision: 0.99, recall: 0.98, f1_score: 0.985, support: 100 },
        low_grade_metrics: ClassMetrics { precision: 0.98, recall: 0.99, f1_score: 0.985, support: 100 },
        confusion_matrix: [[98, 2], [1, 99]],
    };
    
    let mock_power = PowerSummary {
        avg_total_w: 14.5,
        peak_total_w: 19.2,
        avg_cpu_temp: 58.0,
        peak_cpu_temp: 65.0,
        avg_cpu_gpu_w: 8.5,
        peak_cpu_gpu_w: 12.0,
        energy_joules: 350.0,
        duration_secs: 24.5,
    };
    
    let report = TrainingReport::RandomForest {
        train_eval: mock_eval.clone(),
        val_eval: mock_eval.clone(),
        train_accuracy: 0.99,
        val_accuracy: 0.985,
        n_trees: 100,
        training_secs: 24.5,
        accuracy_curve: vec![(1, 0.8, 0.7), (5, 0.9, 0.85), (10, 0.99, 0.985)],
        power: Some(mock_power),
    };
    
    let out_dir = "/home/maulvin/Documents/DGA/src-tauri";
    let _ = generate_training_pdf(&report, out_dir);
    println!("Sample PDF generated at {}", out_dir);
}
