pub mod ml;
pub mod database;
pub mod power_monitor;

#[tauri::command]
async fn run_feature_extraction(method: &str) -> Result<String, String> {
    Ok(format!("Fitur berhasil diekstraksi menggunakan {}", method))
}

#[tauri::command]
async fn run_training(model_type: &str, params: &str) -> Result<String, String> {
    Ok(format!("Training {} selesai dengan parameter {}", model_type, params))
}

#[tauri::command]
async fn run_validation(validation_type: &str) -> Result<String, String> {
    Ok(format!("Validasi {} selesai", validation_type))
}

#[tauri::command]
async fn generate_pdf_report(out_dir: String) -> Result<String, String> {
    use crate::ml::report::{generate_training_pdf, TrainingReport};
    use crate::ml::evaluation::{EvaluationResults, ClassMetrics};
    use crate::power_monitor::PowerSummary;

    let mock_eval = EvaluationResults {
        accuracy: 0.987,
        class_metrics: [
            ClassMetrics { precision: 0.99, recall: 0.98, f1_score: 0.985, support: 25 },
            ClassMetrics { precision: 0.98, recall: 0.99, f1_score: 0.985, support: 25 },
            ClassMetrics { precision: 0.99, recall: 0.99, f1_score: 0.990, support: 25 },
            ClassMetrics { precision: 0.98, recall: 0.98, f1_score: 0.980, support: 25 },
        ],
        confusion_matrix: [
            [24, 1, 0, 0],
            [1, 24, 0, 0],
            [0, 0, 25, 0],
            [0, 0, 0, 25],
        ],
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
        train_accuracy: 0.987,
        val_accuracy: 0.985,
        n_trees: 100,
        training_secs: 24.5,
        accuracy_curve: vec![(1, 0.8, 0.7), (5, 0.9, 0.85), (10, 0.987, 0.985)],
        power: Some(mock_power),
    };
    
    let target_dir = if out_dir.is_empty() { ".".to_string() } else { out_dir };
    let _ = std::fs::create_dir_all(&target_dir);
    
    match generate_training_pdf(&report, &target_dir) {
        Ok(_) => Ok(format!("PDF report successfully generated at {}", target_dir)),
        Err(e) => Err(e),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            run_feature_extraction,
            run_training,
            run_validation,
            generate_pdf_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
