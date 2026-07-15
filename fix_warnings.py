import os

dead_code_files = [
    "src/domain/models.rs",
    "src/domain/enums.rs",
    "src/domain/traits.rs",
    "src/infrastructure/database.rs",
    "src/infrastructure/repositories/session_repo.rs",
    "src/infrastructure/repositories/sensor_data_repo.rs",
    "src/infrastructure/repositories/feature_repo.rs",
    "src/infrastructure/repositories/model_repo.rs",
    "src/infrastructure/repositories/prediction_repo.rs",
    "src/infrastructure/serial/teensy.rs",
    "src/infrastructure/ml/svm.rs",
    "src/infrastructure/ml/random_forest.rs",
    "src/infrastructure/ml/snn.rs",
    "src/infrastructure/ml/persistence.rs",
    "src/application/acquisition_service.rs",
    "src/application/feature_service.rs",
    "src/application/preprocessing_service.rs",
    "src/application/training_service.rs",
    "src/application/inference_service.rs",
    "src/application/export_service.rs",
    "src/state.rs",
    "src/ui/widgets/sensor_card.rs"
]

replacements = {
    "src/infrastructure/repositories/sensor_data_repo.rs": [
        ("use crate::domain::models::{SensorReading, NUM_SENSORS};", "use crate::domain::models::SensorReading;")
    ],
    "src/infrastructure/repositories/model_repo.rs": [
        ("use crate::domain::models::{TrainedModel, TrainingResult, NormalizationParams, FoldResult};", "use crate::domain::models::{TrainedModel, TrainingResult};")
    ],
    "src/infrastructure/repositories/prediction_repo.rs": [
        ("use crate::domain::models::{PredictionLog, SensorReading};", "use crate::domain::models::PredictionLog;")
    ],
    "src/infrastructure/ml/persistence.rs": [
        ("if (!path.exists()) {", "if !path.exists() {")
    ],
    "src/application/preprocessing_service.rs": [
        ("use ndarray::{Array1, Array2, Axis};", "use ndarray::Array2;")
    ],
    "src/application/training_service.rs": [
        ("use crate::domain::models::{ExtractedFeature, TrainedModel, TrainingResult, NormalizationParams};", "use crate::domain::models::{ExtractedFeature, TrainedModel, TrainingResult};")
    ],
    "src/application/inference_service.rs": [
        ("use anyhow::{Result, anyhow};", "use anyhow::Result;"),
        ("use ndarray::{Array1, Array2};", "use ndarray::Array2;")
    ],
    "src/ui/about.rs": [
        ("use egui::{Ui, Color32, Align, Layout, RichText};", "use egui::{Ui, Color32, RichText};"),
        ("use crate::ui::theme::{PANEL_BG, ACCENT_BLUE, TEXT_MUTED};", "use crate::ui::theme::{ACCENT_BLUE, TEXT_MUTED};")
    ],
    "src/ui/widgets/status_led.rs": [
        ("use egui::{Ui, Color32, Vec2, Rect, Sense, Stroke};", "use egui::{Ui, Color32, Vec2, Sense, Stroke};")
    ],
    "src/ui/widgets/sensor_card.rs": [
        ("use egui::{Ui, Color32, Rounding, Stroke, Vec2};", "use egui::{Ui, Color32, Stroke};")
    ],
    "src/ui/widgets/alarm_panel.rs": [
        ("use egui::{Ui, Color32, Rounding, Stroke};", "use egui::{Ui, Color32, Stroke};")
    ],
    "src/infrastructure/ml/snn.rs": [
        ("fn train(&mut self, features: &Array2<f64>, labels: &Array1<usize>) -> Result<()> {", "fn train(&mut self, features: &Array2<f64>, _labels: &Array1<usize>) -> Result<()> {")
    ],
    "src/ui/ml_studio.rs": [
        ("pub fn render(ui: &mut Ui, state: &mut AppState) {", "pub fn render(ui: &mut Ui, _state: &mut AppState) {")
    ],
    "src/ui/classification.rs": [
        ("pub fn render(ui: &mut Ui, state: &mut AppState) {", "pub fn render(ui: &mut Ui, _state: &mut AppState) {")
    ]
}

base_dir = "/home/maulvin/Documents/DGA"

# Apply dead code allows
for rel_path in dead_code_files:
    path = os.path.join(base_dir, rel_path)
    if os.path.exists(path):
        with open(path, "r") as f:
            content = f.read()
        if not content.startswith("#![allow(dead_code)]"):
            with open(path, "w") as f:
                f.write("#![allow(dead_code)]\n" + content)

# Apply specific replacements
for rel_path, reps in replacements.items():
    path = os.path.join(base_dir, rel_path)
    if os.path.exists(path):
        with open(path, "r") as f:
            content = f.read()
        for old, new in reps:
            content = content.replace(old, new)
        with open(path, "w") as f:
            f.write(content)

print("Warnings fixed.")
