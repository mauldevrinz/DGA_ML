-- DGA Electronic Nose Database Schema
-- SQLite with WAL mode for concurrent read/write

-- Acquisition sessions metadata
CREATE TABLE IF NOT EXISTS acquisition_sessions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    label TEXT NOT NULL CHECK(label IN ('baseline','normal','overheating','arcing')),
    started_at TEXT NOT NULL,
    ended_at TEXT,
    sample_count INTEGER DEFAULT 0,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Raw sensor readings (17 channels per row)
CREATE TABLE IF NOT EXISTS sensor_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES acquisition_sessions(id) ON DELETE CASCADE,
    timestamp_ms INTEGER NOT NULL,
    mos_01 REAL NOT NULL DEFAULT 0.0,
    mos_02 REAL NOT NULL DEFAULT 0.0,
    mos_03 REAL NOT NULL DEFAULT 0.0,
    mos_04 REAL NOT NULL DEFAULT 0.0,
    mos_05 REAL NOT NULL DEFAULT 0.0,
    mos_06 REAL NOT NULL DEFAULT 0.0,
    mos_07 REAL NOT NULL DEFAULT 0.0,
    mos_08 REAL NOT NULL DEFAULT 0.0,
    mos_09 REAL NOT NULL DEFAULT 0.0,
    mos_10 REAL NOT NULL DEFAULT 0.0,
    mos_11 REAL NOT NULL DEFAULT 0.0,
    mos_12 REAL NOT NULL DEFAULT 0.0,
    mos_13 REAL NOT NULL DEFAULT 0.0,
    mos_14 REAL NOT NULL DEFAULT 0.0,
    ndir REAL NOT NULL DEFAULT 0.0,
    sht20_temp REAL NOT NULL DEFAULT 0.0,
    sht20_humidity REAL NOT NULL DEFAULT 0.0
);

-- Extracted feature vectors per session
CREATE TABLE IF NOT EXISTS extracted_features (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES acquisition_sessions(id) ON DELETE CASCADE,
    feature_vector TEXT NOT NULL,
    feature_names TEXT NOT NULL,
    label TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Trained model metadata and binary
CREATE TABLE IF NOT EXISTS trained_models (
    id TEXT PRIMARY KEY,
    model_type TEXT NOT NULL CHECK(model_type IN ('svm','random_forest','snn')),
    name TEXT NOT NULL,
    hyperparameters TEXT NOT NULL,
    feature_names TEXT NOT NULL,
    normalization_params TEXT,
    model_binary BLOB,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Training evaluation results
CREATE TABLE IF NOT EXISTS training_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    model_id TEXT NOT NULL REFERENCES trained_models(id) ON DELETE CASCADE,
    accuracy REAL,
    precision_score REAL,
    recall REAL,
    f1_score REAL,
    confusion_matrix TEXT,
    training_time_ms INTEGER,
    inference_time_ms INTEGER,
    memory_usage_bytes INTEGER,
    fold_results TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Real-time prediction log
CREATE TABLE IF NOT EXISTS prediction_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    model_id TEXT NOT NULL REFERENCES trained_models(id) ON DELETE CASCADE,
    predicted_class TEXT NOT NULL,
    confidence REAL,
    probabilities TEXT,
    sensor_snapshot TEXT NOT NULL,
    inference_time_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Performance indexes
CREATE INDEX IF NOT EXISTS idx_sensor_data_session ON sensor_data(session_id);
CREATE INDEX IF NOT EXISTS idx_sensor_data_time ON sensor_data(timestamp_ms);
CREATE INDEX IF NOT EXISTS idx_extracted_features_session ON extracted_features(session_id);
CREATE INDEX IF NOT EXISTS idx_prediction_logs_model ON prediction_logs(model_id);
CREATE INDEX IF NOT EXISTS idx_prediction_logs_time ON prediction_logs(created_at);
