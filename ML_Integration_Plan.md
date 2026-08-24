# DGA ML Integration Plan

## Overview of ML Models
The `ML/src/ml` directory contains custom Rust implementations for several machine learning models designed to classify time-series sensor data (e-nose data, referred to as "Coffee" classification but applied to DGA).
- **Deep Learning (Raw Time-Series):** `CoffeeCNN` and `CoffeeLSTM` process raw 3D data (`Array3<f32>`: samples × channels × timesteps).
- **Classical ML (Feature-Based):** `CoffeeRandomForest`, `CoffeeSVM`, and `CoffeeMLP` use feature vectors. They rely on internal feature extraction (mean, std, min, max, range, median per channel) or external advanced features extracted via a bridge to Python (`tsfresh` bridging through `ML/python/lda_viz.py` and `pca_viz.py`).
- The implementations use pure `ndarray` and SGD (e.g., hinge loss for SVM) rather than relying entirely on `linfa` under the hood.

## Previous Usage in the Rust Backend
Before the transition to Tauri, the app was a native `egui` desktop application (located in `src/main.rs`, `src/app.rs`, and `src/ui/`). The ML models were directly invoked by the UI layer:
- The UI handled training configuration and triggered training loops.
- Training progress was tracked using closure callbacks passed to functions like `fit_with_curve`.
- The UI read these updates each frame from an `AppState` or background thread to render real-time accuracy and loss curves.

## Step-by-Step Integration Plan for Tauri + React

### 1. Initialize `src-tauri/src`
The Tauri backend directory `src-tauri/src` currently does not exist. We need to create it and set up the Tauri 2.0 entry points:
- Create `src-tauri/src/main.rs` to bootstrap the app.
- Create `src-tauri/src/lib.rs` to register plugins and commands.

### 2. Migrate the Rust ML Code
- Move the `ML/src/ml` folder into `src-tauri/src/ml`.
- Ensure `Cargo.toml` in `src-tauri` has the necessary dependencies (`ndarray`, `rand`, `serde`, etc., which are mostly already present).
- Expose the ML module by adding `pub mod ml;` in `src-tauri/src/lib.rs`.

### 3. Implement Tauri Commands
Create Rust functions annotated with `#[tauri::command]` to act as endpoints for the React frontend:
- `load_data()`: Reads DGA data from SQLite (via `sqlx`).
- `train_model(config)`: Spawns a `tokio::spawn` background task to run the training loop (e.g., `model.fit_with_curve()`).
- `predict(sample)`: Runs inference on new sensor data.

### 4. Bridge Real-time Progress via Tauri Events
Because React cannot pass Rust closures for progress updates:
- Refactor the progress callbacks in the training methods (e.g., `fit_with_progress`) to emit Tauri events using `app_handle.emit("training-progress", payload)`.
- The payload should include the current epoch, training loss, training accuracy, and validation accuracy.

### 5. Connect the React Components
In the frontend (`src/components/MLStudio.jsx` and `Classification.jsx`):
- Use `@tauri-apps/api/core` `invoke('train_model', { config })` to start training.
- Use `@tauri-apps/api/event` `listen('training-progress', (event) => { ... })` to update state variables driving the progress bars and real-time loss/accuracy charts.
