#!/usr/bin/env python3
"""
DGA_ML Gas Composition Pipeline — Google Colab
===================================================
Multi-label gas composition prediction + IEC 60599 fault diagnosis.

Gas Classes:
  - Udara Bersih (Clean Air)
  - Asetilena (C2H2 — Acetylene)
  - Etilena (C2H4 — Ethylene)
  - Hidrogen (H2 — Hydrogen)
  - Alcohol

Models: SNN (Spiking Neural Network), Random Forest, SVM
Output: Confidence per gas (0-100%) + Thermal/Electrical Fault classification

Pipeline:
  1. Data Loading & Multi-Label Encoding
  2. TSFRESH Feature Extraction & Selection
  3. Data Augmentation (Synthetic Mixtures)
  4. Multi-Output Model Training (SNN, RF, SVM)
  5. IEC 60599 Fault Diagnosis
  6. Evaluasi: K-Fold, LOMO, LOCO
  7. Model Compression & Export
  8. Predict Unseen Data (Minyak Trafo)
  9. Flask API Server (koneksi frontend)
"""

import os
import json
import glob
import numpy as np
import pandas as pd
import pickle
import time
from pathlib import Path
from copy import deepcopy
import warnings
warnings.filterwarnings('ignore')

# ML Libraries
from sklearn.model_selection import StratifiedKFold, KFold, GridSearchCV, LeaveOneGroupOut
from sklearn.preprocessing import StandardScaler, MinMaxScaler
from sklearn.metrics import accuracy_score, f1_score, hamming_loss, classification_report
from sklearn.svm import SVC
from sklearn.ensemble import RandomForestClassifier
from sklearn.multioutput import MultiOutputClassifier

# PyTorch (for SNN)
import torch
import torch.nn as nn
from torch.utils.data import DataLoader, TensorDataset

# TSFRESH
try:
    from tsfresh import extract_features, select_features
    from tsfresh.utilities.dataframe_functions import impute
    from tsfresh.feature_extraction import MinimalFCParameters, EfficientFCParameters
except ImportError:
    print("MENGINSTALL TSFRESH...")
    os.system("pip install tsfresh")
    from tsfresh import extract_features, select_features
    from tsfresh.utilities.dataframe_functions import impute
    from tsfresh.feature_extraction import MinimalFCParameters, EfficientFCParameters

# snnTorch (for Spiking Neural Network)
try:
    import snntorch as snn
    from snntorch import spikegen
    HAS_SNNTORCH = True
except ImportError:
    print("MENGINSTALL SNNTORCH...")
    os.system("pip install snntorch")
    try:
        import snntorch as snn
        from snntorch import spikegen
        HAS_SNNTORCH = True
    except ImportError:
        HAS_SNNTORCH = False
        print("⚠️ snnTorch tidak tersedia, menggunakan LIF implementation manual.")

# =============================================================================
# CONFIGURATION
# =============================================================================
DATA_ROOT = "data/raw"
UNSEEN_ROOT = "data/unseen"
SENSOR_NAMES = [
    "mos0_mV", "mos1_mV", "mos2_mV", "mos3_mV", "mos4_mV", "mos5_mV", "mos6_mV", "mos7_mV",
    "mos8_mV", "mos9_mV", "mos10_mV", "mos11_mV", "mos12_mV", "mos13_mV", "mos14_mV", "mos15_mV",
    "temp_chamber_C", "temp_oil_C", "humidity_chamber_pct", "humidity_oil_pct",
    "ina226_1_vbus_V", "ina226_1_current_mA", "ina226_1_power_W",
    "ina226_2_vbus_V", "ina226_2_current_mA", "ina226_2_power_W",
    "kria_vbus_V", "kria_current_mA", "kria_power_W",
    "ndir_ratio", "ndir_baseline", "ndir_response_pct", "ndir_absorbance_au", "flow_inlet"
]
N_SENSORS = len(SENSOR_NAMES)
N_TIMESTEPS = 300

GAS_NAMES = ["AIR", "C2H2", "C2H4", "H2", "CH4"]
GAS_FOLDERS = ["AIR", "C2H2", "C2H4", "H2", "CH4"]
N_GASES = 5

# IEC 60599 Fault Categories
IEC_FAULTS = {
    "Normal":  "Tidak ada fault terdeteksi",
    "PD":      "Partial Discharge (Electrical Fault)",
    "D1":      "Low Energy Discharge — Sparking (Electrical Fault)",
    "D2":      "High Energy Discharge — Arcing (Electrical Fault)",
    "T1":      "Thermal Fault < 300°C",
    "T2":      "Thermal Fault 300–700°C",
    "T3":      "Thermal Fault > 700°C",
    "DT":      "Mixed Thermal & Electrical Fault",
}

# =============================================================================
# 1. DATA LOADING & MULTI-LABEL ENCODING
# =============================================================================

def load_data_for_tsfresh(data_root):
    """
    Load data dari 5 folder gas dan encode sebagai multi-label (one-hot).

    Struktur folder:
      data/raw/
        ├── udara_bersih/measurement_1/*.csv  → label [1,0,0,0,0]
        ├── asetilena/measurement_1/*.csv     → label [0,1,0,0,0]
        ├── etilena/measurement_1/*.csv       → label [0,0,1,0,0]
        ├── hidrogen/measurement_1/*.csv      → label [0,0,0,1,0]
        └── alcohol/measurement_1/*.csv       → label [0,0,0,0,1]
    """
    df_list = []
    labels = {}        # sample_id → one-hot vector (5,)
    measurements = {}  # sample_id → measurement folder name (for LOMO)
    gas_sources = {}   # sample_id → gas index (for stratification)

    sample_id = 0

    for gas_idx, gas_folder in enumerate(GAS_FOLDERS):
        gas_path = os.path.join(data_root, gas_folder)
        if not os.path.exists(gas_path):
            print(f"  ⚠️ Folder '{gas_folder}' tidak ditemukan, skip.")
            continue

        # One-hot label untuk gas ini
        one_hot = np.zeros(N_GASES, dtype=np.float32)
        one_hot[gas_idx] = 1.0

        for folder_name in sorted(os.listdir(gas_path)):
            folder_path = os.path.join(gas_path, folder_name)
            if not os.path.isdir(folder_path):
                continue

            csv_files = sorted(glob.glob(os.path.join(folder_path, "*.csv")))
            for csv_file in csv_files:
                try:
                    df = pd.read_csv(csv_file)
                    if len(df) >= N_TIMESTEPS:
                        df = df.iloc[:N_TIMESTEPS].copy()
                        # Pastikan kolom sesuai dengan SENSOR_NAMES
                        # Kolom 0 adalah 'time', maka ambil kolom 1 dst
                        df = df[SENSOR_NAMES].copy()  # Lebih aman pakai nama kolom jika ada headernya, tapi kita asumsikan kolom 1 s.d 34
                        df.columns = SENSOR_NAMES

                        df['id'] = sample_id
                        df['time'] = np.arange(N_TIMESTEPS)
                        df_list.append(df)

                        labels[sample_id] = one_hot.copy()
                        measurements[sample_id] = f"{gas_folder}/{folder_name}"
                        gas_sources[sample_id] = gas_idx
                        sample_id += 1
                except Exception as e:
                    print(f"  Error loading {csv_file}: {e}")

    if not df_list:
        raise ValueError("Data kosong! Pastikan path DATA_ROOT benar dan struktur folder sesuai.")

    df_all = pd.concat(df_list, ignore_index=True)
    Y = pd.DataFrame.from_dict(labels, orient='index', columns=GAS_NAMES).sort_index()
    groups = pd.Series(measurements).sort_index()
    gas_idx_series = pd.Series(gas_sources).sort_index()

    print(f"  ✅ Loaded {sample_id} samples dari {len(GAS_FOLDERS)} gas classes")
    for i, name in enumerate(GAS_NAMES):
        count = (gas_idx_series == i).sum()
        print(f"     {name}: {count} samples")

    return df_all, Y, groups, gas_idx_series


# =============================================================================
# 2. TSFRESH FEATURE EXTRACTION & SELECTION
# =============================================================================

def extract_and_select_tsfresh(df_all, Y, gas_idx_series):
    """
    Extract TSFRESH features lalu select fitur relevan (union dari semua gas).
    """
    print("\n[1/8] 🔄 Extracting TSFRESH Features...")
    extracted_features = extract_features(
        df_all, column_id="id", column_sort="time",
        default_fc_parameters=EfficientFCParameters(),
        disable_progressbar=False
    )
    impute(extracted_features)
    print(f"      Terekstrak {extracted_features.shape[1]} fitur raw.")

    # Feature Selection: union dari fitur relevan untuk masing-masing gas
    print("      Memilih fitur relevan per gas (union selection)...")
    all_selected_cols = set()

    for gas_idx, gas_name in enumerate(GAS_NAMES):
        y_binary = (Y.iloc[:, gas_idx]).astype(int)
        try:
            selected = select_features(extracted_features, y_binary)
            all_selected_cols.update(selected.columns.tolist())
            print(f"        {gas_name}: {selected.shape[1]} fitur relevan")
        except Exception as e:
            print(f"        {gas_name}: selection failed ({e}), pakai semua fitur")
            all_selected_cols.update(extracted_features.columns.tolist())

    # Ambil union fitur
    selected_cols = sorted(all_selected_cols)
    X_selected = extracted_features[selected_cols]
    print(f"      Total fitur setelah union selection: {X_selected.shape[1]}")

    # Simpan daftar fitur
    os.makedirs("models", exist_ok=True)
    with open("models/selected_feature_names.json", "w") as f:
        json.dump({
            "n_features": X_selected.shape[1],
            "feature_names": selected_cols,
            "gas_names": GAS_NAMES
        }, f, indent=2)

    return X_selected


# =============================================================================
# 3. DATA AUGMENTATION — SYNTHETIC GAS MIXTURES
# =============================================================================

def augment_with_mixtures(X, Y, n_mixtures=200, seed=42):
    """
    Buat sample campuran gas sintetis untuk melatih model mengenali mixture.
    Ini penting karena minyak trafo berisi campuran gas, bukan gas murni.
    """
    print("\n[2/8] 🧪 Membuat campuran gas sintetis...")
    rng = np.random.RandomState(seed)
    X_aug, Y_aug = [], []

    X_np = X.values
    Y_np = Y.values

    for _ in range(n_mixtures):
        # Pilih 2-3 gas acak untuk dicampur
        n_mix = rng.randint(2, min(4, len(X_np)))
        indices = rng.choice(len(X_np), n_mix, replace=False)

        # Bobot acak (Dirichlet distribution)
        weights = rng.dirichlet(np.ones(n_mix))

        # Campurkan fitur (weighted average)
        x_mix = np.zeros(X_np.shape[1])
        y_mix = np.zeros(N_GASES)
        for w, idx in zip(weights, indices):
            x_mix += w * X_np[idx]
            y_mix += w * Y_np[idx]

        # Label: gas dianggap present jika kontribusi > 0.15
        y_mix = (y_mix > 0.15).astype(np.float32)

        X_aug.append(x_mix)
        Y_aug.append(y_mix)

    X_augmented = pd.DataFrame(
        np.vstack([X_np, np.array(X_aug)]),
        columns=X.columns
    )
    Y_augmented = pd.DataFrame(
        np.vstack([Y_np, np.array(Y_aug)]),
        columns=Y.columns
    )

    print(f"      Ditambahkan {n_mixtures} sample campuran")
    print(f"      Total dataset: {len(X_augmented)} samples ({len(X)} asli + {n_mixtures} sintetis)")

    return X_augmented, Y_augmented


# =============================================================================
# 4. SPIKING NEURAL NETWORK (SNN) MODEL
# =============================================================================

class SpikingGasNet(nn.Module):
    """
    Spiking Neural Network dengan Leaky Integrate-and-Fire (LIF) neurons
    untuk multi-label gas composition prediction.

    Arsitektur:
      Input (n_features) → FC1 → LIF1 (128) → FC2 → LIF2 (64) → FC3 → LIF3 (5) → Sigmoid
      Rate coding: fitur dikonversi ke spike train proporsional dengan nilainya.
    """
    def __init__(self, n_features, n_gases=5, n_hidden1=128, n_hidden2=64,
                 beta=0.95, n_steps=25):
        super().__init__()
        self.n_steps = n_steps
        self.n_gases = n_gases
        self.beta = beta

        self.fc1 = nn.Linear(n_features, n_hidden1)
        self.fc2 = nn.Linear(n_hidden1, n_hidden2)
        self.fc3 = nn.Linear(n_hidden2, n_gases)

        if HAS_SNNTORCH:
            self.lif1 = snn.Leaky(beta=beta)
            self.lif2 = snn.Leaky(beta=beta)
            self.lif3 = snn.Leaky(beta=beta, output=True)

    def forward(self, x):
        """
        Forward pass: rate-coded spikes → LIF layers → final membrane → sigmoid.
        """
        batch_size = x.shape[0]

        # Normalisasi ke [0, 1] untuk rate coding
        x_rate = torch.sigmoid(x)

        if HAS_SNNTORCH:
            mem1 = self.lif1.init_leaky()
            mem2 = self.lif2.init_leaky()
            mem3 = self.lif3.init_leaky()

            for step in range(self.n_steps):
                # Rate-coded Bernoulli spikes
                spk_in = spikegen.rate(x_rate, num_steps=1).squeeze(0)

                cur1 = self.fc1(spk_in)
                spk1, mem1 = self.lif1(cur1, mem1)

                cur2 = self.fc2(spk1)
                spk2, mem2 = self.lif2(cur2, mem2)

                cur3 = self.fc3(spk2)
                spk3, mem3 = self.lif3(cur3, mem3)

            # Final membrane potential → sigmoid = confidence per gas
            return torch.sigmoid(mem3)
        else:
            # Fallback: Manual LIF implementation
            device = x.device
            mem1 = torch.zeros(batch_size, 128, device=device)
            mem2 = torch.zeros(batch_size, 64, device=device)
            mem3 = torch.zeros(batch_size, self.n_gases, device=device)
            threshold = 1.0

            for step in range(self.n_steps):
                spk_in = (torch.rand_like(x_rate) < x_rate).float()

                cur1 = self.fc1(spk_in)
                mem1 = self.beta * mem1 + cur1
                spk1 = (mem1 > threshold).float()
                mem1 = mem1 * (1.0 - spk1)

                cur2 = self.fc2(spk1)
                mem2 = self.beta * mem2 + cur2
                spk2 = (mem2 > threshold).float()
                mem2 = mem2 * (1.0 - spk2)

                cur3 = self.fc3(spk2)
                mem3 = self.beta * mem3 + cur3

            return torch.sigmoid(mem3)


def train_snn(X_train, Y_train, X_val, Y_val, n_features,
              n_epochs=100, lr=1e-3, batch_size=32):
    """Train SNN dengan BCE loss untuk multi-label prediction."""

    device = torch.device('cuda' if torch.cuda.is_available() else 'cpu')
    print(f"      Perangkat: {device}")

    model = SpikingGasNet(n_features, N_GASES).to(device)
    optimizer = torch.optim.Adam(model.parameters(), lr=lr)
    criterion = nn.BCELoss()

    # DataLoader
    X_t = torch.FloatTensor(X_train).to(device)
    Y_t = torch.FloatTensor(Y_train).to(device)
    dataset = TensorDataset(X_t, Y_t)
    loader = DataLoader(dataset, batch_size=batch_size, shuffle=True)

    X_v = torch.FloatTensor(X_val).to(device)
    Y_v = torch.FloatTensor(Y_val).to(device)

    best_loss = float('inf')
    best_state = None
    patience_counter = 0
    patience = 15

    for epoch in range(n_epochs):
        model.train()
        epoch_loss = 0
        for batch_X, batch_Y in loader:
            optimizer.zero_grad()
            preds = model(batch_X)
            loss = criterion(preds, batch_Y)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 5.0)
            optimizer.step()
            epoch_loss += loss.item()

        # Validation
        model.eval()
        with torch.no_grad():
            val_preds = model(X_v)
            val_loss = criterion(val_preds, Y_v).item()

        if val_loss < best_loss:
            best_loss = val_loss
            best_state = deepcopy(model.state_dict())
            patience_counter = 0
        else:
            patience_counter += 1

        if (epoch + 1) % 20 == 0:
            print(f"        Epoch {epoch+1}/{n_epochs} — Train Loss: {epoch_loss/len(loader):.4f}, Val Loss: {val_loss:.4f}")

        if patience_counter >= patience:
            print(f"        Early stopping di epoch {epoch+1}")
            break

    model.load_state_dict(best_state)
    return model


# =============================================================================
# 5. MULTI-OUTPUT MODEL TRAINING (SNN + RF + SVM)
# =============================================================================

def train_all_models(X, Y, gas_idx_series):
    """
    Train 3 model: SNN, Random Forest, SVM.
    Semua model output confidence per gas (multi-label).
    """
    print("\n[3/8] ⚙️ Training Multi-Output Models...")

    scaler = StandardScaler()
    X_scaled = scaler.fit_transform(X)

    # Split train/val (80/20)
    n_train = int(len(X) * 0.8)
    idx = np.random.RandomState(42).permutation(len(X))
    train_idx, val_idx = idx[:n_train], idx[n_train:]

    X_train_sc = X_scaled[train_idx]
    X_val_sc = X_scaled[val_idx]
    Y_train = Y.values[train_idx]
    Y_val = Y.values[val_idx]

    results = {}

    # ---- 1. SPIKING NEURAL NETWORK ----
    print("\n   🧠 [1/3] Training Spiking Neural Network (LIF)...")
    start_snn = time.time()
    snn_model = train_snn(
        X_train_sc, Y_train, X_val_sc, Y_val,
        n_features=X_scaled.shape[1],
        n_epochs=100, lr=1e-3, batch_size=32
    )
    snn_time = time.time() - start_snn

    # Evaluate SNN
    snn_model.eval()
    start_infer = time.time()
    with torch.no_grad():
        device = next(snn_model.parameters()).device
        snn_preds = snn_model(torch.FloatTensor(X_val_sc).to(device)).cpu().numpy()
    snn_infer_ms = (time.time() - start_infer) / len(X_val_sc) * 1000
    
    snn_binary = (snn_preds > 0.5).astype(int)
    snn_acc = accuracy_score(Y_val, snn_binary)
    snn_f1 = f1_score(Y_val, snn_binary, average='macro')
    snn_hamming = hamming_loss(Y_val, snn_binary)
    print(f"      SNN — Subset Acc: {snn_acc:.4f} | Macro F1: {snn_f1:.4f} | Hamming Loss: {snn_hamming:.4f} | Waktu: {snn_time:.2f}s | Inferensi: {snn_infer_ms:.2f}ms")
    results['snn'] = {'model': snn_model, 'acc': snn_acc, 'f1': snn_f1, 'hamming': snn_hamming, 'time': snn_time, 'infer_ms': snn_infer_ms}

    # ---- 2. RANDOM FOREST ----
    print("\n   🌲 [2/3] Training Random Forest (Multi-Output)...")
    param_grid_rf = {
        'estimator__n_estimators': [100, 200],
        'estimator__max_depth': [10, 20, None]
    }
    rf_base = MultiOutputClassifier(RandomForestClassifier(random_state=42))
    rf_cv = GridSearchCV(rf_base, param_grid_rf, cv=3, scoring='accuracy', n_jobs=-1)
    
    start_rf = time.time()
    rf_cv.fit(X_train_sc, Y_train)
    rf_time = time.time() - start_rf
    
    rf_model = rf_cv.best_estimator_
    print(f"      RF Best Params: {rf_cv.best_params_}")

    start_infer = time.time()
    rf_preds_binary = rf_model.predict(X_val_sc)
    rf_infer_ms = (time.time() - start_infer) / len(X_val_sc) * 1000
    
    # Get probabilities per gas
    rf_preds_proba = np.column_stack([
        est.predict_proba(X_val_sc)[:, 1] if est.predict_proba(X_val_sc).shape[1] > 1
        else est.predict_proba(X_val_sc)[:, 0]
        for est in rf_model.estimators_
    ])
    rf_acc = accuracy_score(Y_val, rf_preds_binary)
    rf_f1 = f1_score(Y_val, rf_preds_binary, average='macro')
    rf_hamming = hamming_loss(Y_val, rf_preds_binary)
    print(f"      RF  — Subset Acc: {rf_acc:.4f} | Macro F1: {rf_f1:.4f} | Hamming Loss: {rf_hamming:.4f} | Waktu: {rf_time:.2f}s | Inferensi: {rf_infer_ms:.2f}ms")
    results['rf'] = {'model': rf_model, 'acc': rf_acc, 'f1': rf_f1, 'hamming': rf_hamming, 'time': rf_time, 'infer_ms': rf_infer_ms}

    # ---- 3. SVM ----
    print("\n   📐 [3/3] Training SVM (Multi-Output, RBF)...")
    param_grid_svm = {
        'estimator__C': [1, 10],
        'estimator__kernel': ['rbf']
    }
    svm_base = MultiOutputClassifier(SVC(probability=True, random_state=42))
    svm_cv = GridSearchCV(svm_base, param_grid_svm, cv=3, scoring='accuracy', n_jobs=-1)
    
    start_svm = time.time()
    svm_cv.fit(X_train_sc, Y_train)
    svm_time = time.time() - start_svm
    
    svm_model = svm_cv.best_estimator_
    print(f"      SVM Best Params: {svm_cv.best_params_}")

    start_infer = time.time()
    svm_preds_binary = svm_model.predict(X_val_sc)
    svm_infer_ms = (time.time() - start_infer) / len(X_val_sc) * 1000
    
    svm_preds_proba = np.column_stack([
        est.predict_proba(X_val_sc)[:, 1] if est.predict_proba(X_val_sc).shape[1] > 1
        else est.predict_proba(X_val_sc)[:, 0]
        for est in svm_model.estimators_
    ])
    svm_acc = accuracy_score(Y_val, svm_preds_binary)
    svm_f1 = f1_score(Y_val, svm_preds_binary, average='macro')
    svm_hamming = hamming_loss(Y_val, svm_preds_binary)
    print(f"      SVM — Subset Acc: {svm_acc:.4f} | Macro F1: {svm_f1:.4f} | Hamming Loss: {svm_hamming:.4f} | Waktu: {svm_time:.2f}s | Inferensi: {svm_infer_ms:.2f}ms")
    results['svm'] = {'model': svm_model, 'acc': svm_acc, 'f1': svm_f1, 'hamming': svm_hamming, 'time': svm_time, 'infer_ms': svm_infer_ms}

    # Summary
    print("\n   📊 Ringkasan Perbandingan Model:")
    print(f"   {'Model':<10} {'Subset Acc':<12} {'Macro F1':<10} {'Hamming':<10} {'Train Time':<12} {'Inference/Sample'}")
    print(f"   {'─'*80}")
    for name in ['snn', 'rf', 'svm']:
        r = results[name]
        star = " ⭐" if r['f1'] == max(results[n]['f1'] for n in results) else ""
        print(f"   {name.upper():<10} {r['acc']:<12.4f} {r['f1']:<10.4f} {r['hamming']:<10.4f} {r['time']:<12.2f} {r['infer_ms']:.3f} ms{star}")

    # Detailed report for Random Forest (sebagai contoh model yang sering terbaik)
    print("\n   📋 Laporan Detail per Gas (Random Forest):")
    print(classification_report(Y_val, rf_preds_binary, target_names=GAS_NAMES, zero_division=0))

    # Tampilkan contoh confidence score (probabilitas) untuk 2 sampel pertama
    print("\n   🔍 Contoh Output Confidence (Probabilitas) Multi-Label - Random Forest:")
    for i in range(min(2, len(X_val_sc))):
        print(f"      Sampel {i+1}:")
        print(f"      - Label Asli (Ground Truth) : {dict(zip(GAS_NAMES, Y_val[i]))}")
        
        # Format persentase untuk probabilitas
        confidences = {gas: f"{prob*100:.1f}%" for gas, prob in zip(GAS_NAMES, rf_preds_proba[i])}
        print(f"      - Prediksi Confidence Model : {confidences}\n")

    return results, scaler


# =============================================================================
# 6. IEC 60599 FAULT DIAGNOSIS
# =============================================================================

def diagnose_iec60599(gas_confidences):
    """
    Rule-based IEC 60599 fault diagnosis berdasarkan confidence gas.

    Input: dict dengan keys 'Asetilena', 'Etilena', 'Hidrogen' (confidence 0-100)
    Output: (fault_code, fault_type, description, severity)

    Referensi:
      - IEC 60599:2022 — Mineral oil-filled electrical equipment in service
      - Berdasarkan Key Gas Method dan rasio gas
    """
    # Ambil confidence (0-100 scale)
    c2h2 = gas_confidences.get('C2H2', 0)    # Acetylene
    c2h4 = gas_confidences.get('C2H4', 0)    # Ethylene
    h2   = gas_confidences.get('H2', 0)      # Hydrogen
    ch4  = gas_confidences.get('CH4', 0)     # Methane
    udara = gas_confidences.get('AIR', 0)    # Clean Air

    # Threshold: gas dianggap "signifikan" jika confidence > 30%
    SIGNIFICANT = 30

    # Jika semua gas rendah atau udara bersih dominan
    if udara > 70 and c2h2 < SIGNIFICANT and c2h4 < SIGNIFICANT and h2 < SIGNIFICANT:
        return {
            "fault_code": "Normal",
            "fault_type": "No Fault",
            "description": IEC_FAULTS["Normal"],
            "severity": "🟢 Normal",
            "category": "Normal"
        }

    # Hitung rasio kunci IEC 60599
    # R1 = C2H2/C2H4 (Acetylene/Ethylene ratio)
    r1 = c2h2 / max(c2h4, 1)

    # Hitung proporsi relatif dari fault gases
    fault_total = c2h2 + c2h4 + h2
    if fault_total < 10:
        return {
            "fault_code": "Normal",
            "fault_type": "No Fault",
            "description": "Konsentrasi fault gas terlalu rendah",
            "severity": "🟢 Normal",
            "category": "Normal"
        }

    p_c2h2 = c2h2 / fault_total  # Proporsi Asetilena
    p_c2h4 = c2h4 / fault_total  # Proporsi Etilena
    p_h2   = h2 / fault_total    # Proporsi Hidrogen

    # === ELECTRICAL FAULTS ===
    # D2: High Energy Discharge (Arcing)
    # Karakteristik: C2H2 dominan + H2 signifikan
    if p_c2h2 > 0.40 and h2 > SIGNIFICANT:
        return {
            "fault_code": "D2",
            "fault_type": "Electrical Fault",
            "description": IEC_FAULTS["D2"],
            "severity": "🔴 Critical",
            "category": "Electrical",
            "detail": f"Rasio C2H2/C2H4 = {r1:.2f} (>1 = discharge)"
        }

    # D1: Low Energy Discharge (Sparking)
    # Karakteristik: C2H2 moderate, H2 moderate
    if p_c2h2 > 0.25 and c2h2 > SIGNIFICANT:
        return {
            "fault_code": "D1",
            "fault_type": "Electrical Fault",
            "description": IEC_FAULTS["D1"],
            "severity": "🟠 Warning",
            "category": "Electrical",
            "detail": f"Rasio C2H2/C2H4 = {r1:.2f}"
        }

    # PD: Partial Discharge
    # Karakteristik: H2 dominan, C2H2 rendah
    if p_h2 > 0.50 and c2h2 < SIGNIFICANT:
        return {
            "fault_code": "PD",
            "fault_type": "Electrical Fault",
            "description": IEC_FAULTS["PD"],
            "severity": "🟡 Caution",
            "category": "Electrical",
            "detail": f"H2 proporsi = {p_h2:.1%}"
        }

    # === THERMAL FAULTS ===
    # T3: Thermal Fault > 700°C
    # Karakteristik: C2H4 sangat dominan
    if p_c2h4 > 0.60 and c2h4 > 60:
        return {
            "fault_code": "T3",
            "fault_type": "Thermal Fault",
            "description": IEC_FAULTS["T3"],
            "severity": "🔴 Critical",
            "category": "Thermal",
            "detail": f"C2H4 dominan ({c2h4:.0f}%) — overheating parah"
        }

    # T2: Thermal Fault 300-700°C
    if p_c2h4 > 0.40 and c2h4 > SIGNIFICANT:
        return {
            "fault_code": "T2",
            "fault_type": "Thermal Fault",
            "description": IEC_FAULTS["T2"],
            "severity": "🟠 Warning",
            "category": "Thermal",
            "detail": f"C2H4 proporsi = {p_c2h4:.1%}"
        }

    # T1: Thermal Fault < 300°C
    if p_c2h4 > 0.20 or c2h4 > SIGNIFICANT:
        return {
            "fault_code": "T1",
            "fault_type": "Thermal Fault",
            "description": IEC_FAULTS["T1"],
            "severity": "🟡 Caution",
            "category": "Thermal"
        }

    # DT: Mixed Thermal & Electrical
    if c2h2 > SIGNIFICANT and c2h4 > SIGNIFICANT:
        return {
            "fault_code": "DT",
            "fault_type": "Mixed Fault",
            "description": IEC_FAULTS["DT"],
            "severity": "🔴 Critical",
            "category": "Mixed",
            "detail": f"C2H2={c2h2:.0f}% + C2H4={c2h4:.0f}%"
        }

    # Default
    return {
        "fault_code": "T1",
        "fault_type": "Thermal Fault",
        "description": IEC_FAULTS["T1"],
        "severity": "🟡 Caution",
        "category": "Thermal"
    }


# =============================================================================
# 7. EVALUATION — K-Fold, LOMO, LOCO
# =============================================================================

def evaluate_kfold(X, Y, model_fn, scaler_fn, model_name, n_splits=5):
    """K-Fold Cross Validation untuk multi-label."""
    print(f"\n   📊 K-Fold CV ({n_splits} folds) — {model_name}")
    kf = KFold(n_splits=n_splits, shuffle=True, random_state=42)
    all_acc, all_f1 = [], []

    for fold, (train_idx, test_idx) in enumerate(kf.split(X)):
        sc = StandardScaler()
        X_train_sc = sc.fit_transform(X.values[train_idx])
        X_test_sc = sc.transform(X.values[test_idx])
        Y_train = Y.values[train_idx]
        Y_test = Y.values[test_idx]

        model = model_fn()
        model.fit(X_train_sc, Y_train)
        preds = model.predict(X_test_sc)

        acc = accuracy_score(Y_test, preds)
        f1 = f1_score(Y_test, preds, average='macro')
        all_acc.append(acc)
        all_f1.append(f1)

    print(f"      Acc: {np.mean(all_acc):.4f} ± {np.std(all_acc):.4f}")
    print(f"      F1:  {np.mean(all_f1):.4f} ± {np.std(all_f1):.4f}")
    return np.mean(all_acc), np.mean(all_f1)


def evaluate_lomo(X, Y, groups, model_fn, model_name):
    """Leave-One-Measurement-Out evaluation."""
    print(f"\n   📊 LOMO — {model_name}")
    logo = LeaveOneGroupOut()
    all_acc, all_f1 = [], []

    for train_idx, test_idx in logo.split(X, Y, groups):
        sc = StandardScaler()
        X_train_sc = sc.fit_transform(X.values[train_idx])
        X_test_sc = sc.transform(X.values[test_idx])
        Y_train = Y.values[train_idx]
        Y_test = Y.values[test_idx]

        model = model_fn()
        model.fit(X_train_sc, Y_train)
        preds = model.predict(X_test_sc)

        acc = accuracy_score(Y_test, preds)
        f1 = f1_score(Y_test, preds, average='macro', zero_division=0)
        all_acc.append(acc)
        all_f1.append(f1)

    print(f"      LOMO Acc: {np.mean(all_acc):.4f} ± {np.std(all_acc):.4f}")
    print(f"      LOMO F1:  {np.mean(all_f1):.4f} ± {np.std(all_f1):.4f}")


def evaluate_loco(df_all, Y, gas_idx_series, model_name="RF"):
    """Leave-One-Channel-Out — test pentingnya tiap sensor."""
    print(f"\n[5/8] 🚨 LOCO (Leave-One-Channel-Out) — {model_name}...")
    baseline_cols = None

    for sensor in SENSOR_NAMES:
        df_dropped = df_all.copy()
        df_dropped[sensor] = 0.0  # Simulasi sensor mati

        feats = extract_features(df_dropped, column_id="id", column_sort="time",
                                 default_fc_parameters=MinimalFCParameters(),
                                 disable_progressbar=True)
        impute(feats)

        rf = MultiOutputClassifier(RandomForestClassifier(n_estimators=50, random_state=42))
        kf = KFold(n_splits=3, shuffle=True, random_state=42)
        accs = []

        for train_idx, test_idx in kf.split(feats, gas_idx_series):
            sc = StandardScaler()
            X_tr = sc.fit_transform(feats.values[train_idx])
            X_te = sc.transform(feats.values[test_idx])
            rf.fit(X_tr, Y.values[train_idx])
            preds = rf.predict(X_te)
            accs.append(accuracy_score(Y.values[test_idx], preds))

        print(f"      Tanpa {sensor:>10}: Acc = {np.mean(accs):.4f}")


def run_all_evaluations(X, Y, groups, gas_idx_series):
    """Jalankan semua evaluasi untuk RF dan SVM."""
    print("\n[4/8] 📊 Running Comprehensive Evaluations...")

    # Model factories
    rf_fn = lambda: MultiOutputClassifier(RandomForestClassifier(n_estimators=100, random_state=42))
    svm_fn = lambda: MultiOutputClassifier(SVC(probability=True, C=10, kernel='rbf', random_state=42))

    # K-Fold
    evaluate_kfold(X, Y, rf_fn, StandardScaler, "Random Forest")
    evaluate_kfold(X, Y, svm_fn, StandardScaler, "SVM")

    # LOMO
    evaluate_lomo(X, Y, groups, rf_fn, "Random Forest")
    evaluate_lomo(X, Y, groups, svm_fn, "SVM")


# =============================================================================
# 8. MODEL COMPRESSION & EXPORT
# =============================================================================

def compress_and_export_snn(snn_model, scaler, filepath):
    """Export SNN ke JSON yang bisa diload oleh Rust (Kria)."""
    print("\n[6/8] 📦 Compressing & Exporting SNN...")

    state = snn_model.state_dict()

    # Ambil weights & biases dari setiap FC layer
    layers_info = []
    for name in ['fc1', 'fc2', 'fc3']:
        w = state[f'{name}.weight'].cpu().numpy()
        b = state[f'{name}.bias'].cpu().numpy()

        # Pruning: set bobot kecil ke 0
        threshold = 1e-3
        w_pruned = np.where(np.abs(w) < threshold, 0.0, w)
        sparsity = np.sum(w_pruned == 0) / w.size * 100

        # Float truncation
        w_rounded = np.round(w_pruned, decimals=4)
        b_rounded = np.round(b, decimals=4)

        layers_info.append({
            'weights': w_rounded.tolist(),
            'bias': b_rounded.tolist(),
            'in_size': int(w.shape[1]),
            'out_size': int(w.shape[0]),
            'sparsity': f"{sparsity:.1f}%"
        })

    model_dict = {
        "model_type": "SNN_LIF",
        "gas_names": GAS_NAMES,
        "config": {
            "n_hidden1": layers_info[0]['out_size'],
            "n_hidden2": layers_info[1]['out_size'],
            "n_outputs": N_GASES,
            "beta": snn_model.beta,
            "n_steps": snn_model.n_steps,
            "threshold": 1.0,
            "output_activation": "sigmoid"
        },
        "layer1": {
            "weights": layers_info[0]['weights'],
            "bias": layers_info[0]['bias'],
            "in_size": layers_info[0]['in_size'],
            "out_size": layers_info[0]['out_size'],
            "use_lif": True
        },
        "layer2": {
            "weights": layers_info[1]['weights'],
            "bias": layers_info[1]['bias'],
            "in_size": layers_info[1]['in_size'],
            "out_size": layers_info[1]['out_size'],
            "use_lif": True
        },
        "layer_out": {
            "weights": layers_info[2]['weights'],
            "bias": layers_info[2]['bias'],
            "in_size": layers_info[2]['in_size'],
            "out_size": layers_info[2]['out_size'],
            "use_lif": True
        },
        "norm": {
            "mean": np.round(scaler.mean_, decimals=4).tolist(),
            "std": np.round(scaler.scale_, decimals=4).tolist()
        }
    }

    os.makedirs(os.path.dirname(filepath) if os.path.dirname(filepath) else "models", exist_ok=True)
    with open(filepath, 'w') as f:
        json.dump(model_dict, f, separators=(',', ':'))

    size_kb = os.path.getsize(filepath) / 1024
    print(f"      ✅ SNN model disimpan: {filepath} ({size_kb:.1f} KB)")
    for i, info in enumerate(layers_info):
        print(f"         Layer {i+1}: {info['in_size']}→{info['out_size']} (sparsity: {info['sparsity']})")

    return model_dict


def export_sklearn_models(rf_model, svm_model, scaler):
    """Export RF dan SVM menggunakan pickle/joblib."""
    print("\n[7/8] 📦 Exporting RF & SVM...")
    os.makedirs("models", exist_ok=True)

    with open("models/rf_model.pkl", "wb") as f:
        pickle.dump({'model': rf_model, 'scaler': scaler}, f)
    print("      ✅ Random Forest → models/rf_model.pkl")

    with open("models/svm_model.pkl", "wb") as f:
        pickle.dump({'model': svm_model, 'scaler': scaler}, f)
    print("      ✅ SVM → models/svm_model.pkl")


# =============================================================================
# 9. PREDICT UNSEEN DATA (MINYAK TRAFO)
# =============================================================================

def predict_unseen(csv_path, model, scaler, model_type='snn'):
    """
    Prediksi komposisi gas dari sample minyak trafo (unseen data).

    Returns:
      gas_confidences: dict {gas_name: confidence_pct}
      iec_diagnosis: dict dari diagnose_iec60599()
    """
    print(f"\n🛢️ Prediksi Unseen: {csv_path}")

    # Load CSV
    df = pd.read_csv(csv_path)
    if len(df) >= N_TIMESTEPS:
        df = df.iloc[:N_TIMESTEPS].copy()
    df = df.iloc[:, 1:N_SENSORS + 1]
    df.columns = SENSOR_NAMES

    # Format untuk TSFRESH
    df['id'] = 0
    df['time'] = np.arange(len(df))

    # Extract features
    features = extract_features(df, column_id="id", column_sort="time",
                                default_fc_parameters=EfficientFCParameters(),
                                disable_progressbar=True)
    impute(features)

    # Load feature names dan align columns
    with open("models/selected_feature_names.json", "r") as f:
        feat_info = json.load(f)
    selected_cols = feat_info["feature_names"]

    # Align: pastikan kolom sama, isi 0 jika tidak ada
    for col in selected_cols:
        if col not in features.columns:
            features[col] = 0.0
    features = features[selected_cols]

    # Scale
    X_scaled = scaler.transform(features.values)

    # Predict
    if model_type == 'snn':
        model.eval()
        with torch.no_grad():
            device = next(model.parameters()).device
            preds = model(torch.FloatTensor(X_scaled).to(device)).cpu().numpy()[0]
    else:
        # RF atau SVM: ambil probabilitas
        preds = np.array([
            est.predict_proba(X_scaled)[0, 1] if est.predict_proba(X_scaled).shape[1] > 1
            else est.predict_proba(X_scaled)[0, 0]
            for est in model.estimators_
        ])

    # Confidence per gas (0-100%)
    gas_confidences = {}
    for i, name in enumerate(GAS_NAMES):
        conf = float(preds[i]) * 100
        gas_confidences[name] = round(conf, 1)

    # IEC 60599 Diagnosis
    iec_result = diagnose_iec60599(gas_confidences)

    # Print hasil
    print("\n   ╔══════════════════════════════════════════════╗")
    print("   ║     HASIL ANALISIS GAS — Virtual GC          ║")
    print("   ╠══════════════════════════════════════════════╣")
    for name, conf in gas_confidences.items():
        bar = "█" * int(conf / 5) + "░" * (20 - int(conf / 5))
        print(f"   ║  {name:<15} {bar} {conf:>5.1f}%  ║")
    print("   ╠══════════════════════════════════════════════╣")
    print(f"   ║  IEC 60599: {iec_result['fault_code']:<6} — {iec_result['fault_type']:<18} ║")
    print(f"   ║  Severity:  {iec_result['severity']:<30}   ║")
    print(f"   ║  {iec_result['description']:<44} ║")
    print("   ╚══════════════════════════════════════════════╝")

    return gas_confidences, iec_result


# =============================================================================
# 10. FLASK API SERVER (untuk koneksi Frontend)
# =============================================================================

def start_api_server(snn_model, rf_model, svm_model, scaler):
    """
    Jalankan Flask API server di Google Colab.
    Frontend bisa POST data sensor → mendapat hasil prediksi + IEC 60599.
    """
    try:
        from flask import Flask, request, jsonify
        from flask_cors import CORS
    except ImportError:
        os.system("pip install flask flask-cors")
        from flask import Flask, request, jsonify
        from flask_cors import CORS

    app = Flask(__name__)
    CORS(app)

    @app.route('/predict', methods=['POST'])
    def predict():
        """
        POST /predict
        Body (JSON): {
          "sensor_data": [[8 values] x 300 timesteps],
          "model": "snn" | "rf" | "svm"
        }
        Response: {
          "gas_confidences": {"Udara_Bersih": 15.2, "Asetilena": 82.3, ...},
          "iec_diagnosis": {...},
          "model_used": "snn"
        }
        """
        data = request.json
        sensor_data = np.array(data.get('sensor_data', []))
        model_choice = data.get('model', 'snn')

        # Validate
        if sensor_data.ndim != 2 or sensor_data.shape[1] != N_SENSORS:
            return jsonify({"error": f"Data harus berukuran (N, {N_SENSORS})"}), 400

        # Format untuk TSFRESH
        df = pd.DataFrame(sensor_data[:N_TIMESTEPS], columns=SENSOR_NAMES)
        df['id'] = 0
        df['time'] = np.arange(len(df))

        # Extract features
        features = extract_features(df, column_id="id", column_sort="time",
                                    default_fc_parameters=EfficientFCParameters(),
                                    disable_progressbar=True)
        impute(features)

        # Align columns
        with open("models/selected_feature_names.json", "r") as f:
            feat_info = json.load(f)
        selected_cols = feat_info["feature_names"]

        for col in selected_cols:
            if col not in features.columns:
                features[col] = 0.0
        features = features[selected_cols]

        X_scaled = scaler.transform(features.values)

        # Predict
        if model_choice == 'snn':
            snn_model.eval()
            with torch.no_grad():
                device = next(snn_model.parameters()).device
                preds = snn_model(torch.FloatTensor(X_scaled).to(device)).cpu().numpy()[0]
        elif model_choice == 'rf':
            preds = np.array([
                est.predict_proba(X_scaled)[0, 1]
                for est in rf_model.estimators_
            ])
        else:  # svm
            preds = np.array([
                est.predict_proba(X_scaled)[0, 1]
                for est in svm_model.estimators_
            ])

        # Build response
        gas_confidences = {}
        for i, name in enumerate(GAS_NAMES):
            gas_confidences[name] = round(float(preds[i]) * 100, 1)

        iec_result = diagnose_iec60599(gas_confidences)

        return jsonify({
            "gas_confidences": gas_confidences,
            "iec_diagnosis": iec_result,
            "model_used": model_choice
        })

    @app.route('/health', methods=['GET'])
    def health():
        return jsonify({"status": "ok", "models": ["snn", "rf", "svm"], "gas_names": GAS_NAMES})

    # Expose via ngrok (untuk Colab)
    try:
        from pyngrok import ngrok
        public_url = ngrok.connect(5000)
        print(f"\n🌐 API tersedia di: {public_url}")
        print(f"   Health check:  {public_url}/health")
        print(f"   Predict:       POST {public_url}/predict")
    except ImportError:
        os.system("pip install pyngrok")
        from pyngrok import ngrok
        public_url = ngrok.connect(5000)
        print(f"\n🌐 API tersedia di: {public_url}")

    app.run(port=5000)


# =============================================================================
# MAIN EXECUTION
# =============================================================================
if __name__ == "__main__":
    print("=" * 60)
    print("🚀 DGA_ML GAS COMPOSITION PIPELINE")
    print("   Multi-Label Prediction + IEC 60599 Fault Diagnosis")
    print("   Models: SNN (LIF) | Random Forest | SVM")
    print("=" * 60)

    # --- 1. Load Data ---
    print("\n[0/8] 📂 Loading Dataset...")
    df_all, Y, groups, gas_idx_series = load_data_for_tsfresh(DATA_ROOT)

    # --- 2. TSFRESH Feature Extraction ---
    X_features = extract_and_select_tsfresh(df_all, Y, gas_idx_series)

    # --- 3. Data Augmentation ---
    X_augmented, Y_augmented = augment_with_mixtures(X_features, Y, n_mixtures=200)

    # --- 4. Train Models ---
    results, scaler = train_all_models(X_augmented, Y_augmented, gas_idx_series)

    # --- 5. Evaluations ---
    run_all_evaluations(X_features, Y, groups, gas_idx_series)

    # --- 6. LOCO ---
    evaluate_loco(df_all, Y, gas_idx_series)

    # --- 7. Export Models ---
    compress_and_export_snn(results['snn']['model'], scaler, "models/snn_gas_composition.json")
    export_sklearn_models(results['rf']['model'], results['svm']['model'], scaler)

    # --- 8. Predict Unseen (jika ada) ---
    unseen_files = glob.glob(os.path.join(UNSEEN_ROOT, "*.csv"))
    if unseen_files:
        print(f"\n[8/8] 🛢️ Prediksi {len(unseen_files)} sample minyak trafo...")
        for csv_file in unseen_files:
            predict_unseen(csv_file, results['snn']['model'], scaler, model_type='snn')
    else:
        print(f"\n[8/8] ⚠️ Tidak ada file unseen di {UNSEEN_ROOT}/")
        print("      Letakkan CSV minyak trafo di folder tersebut, lalu jalankan ulang.")

    print("\n" + "=" * 60)
    print("✅ PIPELINE SELESAI!")
    print("   Model tersimpan di folder models/")
    print("   Jalankan start_api_server() untuk aktifkan API → Frontend")
    print("=" * 60)

    # --- Optional: Start API Server ---
    # Uncomment untuk menjalankan Flask server
    # start_api_server(
    #     results['snn']['model'],
    #     results['rf']['model'],
    #     results['svm']['model'],
    #     scaler
    # )
