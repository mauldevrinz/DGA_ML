"""
tsfresh_pipeline.py
-------------------
Pipeline ekstraksi dan seleksi fitur TSFRESH untuk klasifikasi
kopi Arabica high grade vs low grade dari data e-nose 8 sensor.

Alur:
  1. Load semua CSV dari data/processed/
  2. Ekstraksi fitur dengan TSFRESH (EfficientFCParameters)
  3. Seleksi fitur dengan FRESH + FDR (Benjamini-Yekutieli)
  4. Pruning fitur redundan (korelasi Pearson > 0.95)
  5. Simpan hasil ke data/features/

Output yang dihasilkan:
  data/features/features_raw.csv          <- semua fitur sebelum seleksi
  data/features/features_selected.csv     <- setelah FRESH + FDR
  data/features/features_final.csv        <- setelah pruning korelasi (INPUT MODEL)
  data/features/labels.csv               <- label 0=low_grade, 1=high_grade
  data/features/selected_feature_names.json  <- nama fitur terpilih (dibaca Rust)
  data/features/feature_report.txt        <- ringkasan analisis

Cara run:
  cd MachineLearningModel
  python tools/python/tsfresh_pipeline.py

Install dependency:
  pip install tsfresh pandas numpy scikit-learn
"""

import os
import json
import glob
import time
import numpy as np
import pandas as pd
from pathlib import Path

# ── Konfigurasi ──────────────────────────────────────────────────────────────
PROCESSED_DIR = "data/processed"   # relatif dari root project
FEATURES_DIR  = "data/features"
FDR_LEVEL     = 0.05               # False Discovery Rate (5%)
CORR_THRESHOLD = 0.95              # Threshold pruning korelasi
TOP_N_FEATURES = 100               # Batas maksimal fitur final yang dikirim ke Rust
SENSOR_COLS   = ["tgs2600", "mq135", "mq3", "mq6", "mq7", "tgs2602", "tgs2611", "tgs2620"]
# ─────────────────────────────────────────────────────────────────────────────


def load_all_samples(processed_dir: str):
    """
    Load semua CSV dari data/processed/high_grade dan low_grade.
    Return:
        df_tsfresh : DataFrame format panjang untuk TSFRESH
                     kolom: [id, time, tgs2600, mq135, ...]
        labels     : Series, index=sample_id, value=0/1
        sample_meta: list of dict {id, grade, sample_folder, filename}
    """
    print("\n[1/5] Loading semua CSV dari data/processed/ ...")

    records = []
    label_map = {}
    sample_meta = []
    sample_id = 0

    for grade, label in [("high_grade", 1), ("low_grade", 0)]:
        grade_dir = os.path.join(processed_dir, grade)
        if not os.path.exists(grade_dir):
            print(f"  PERINGATAN: folder tidak ditemukan: {grade_dir}")
            continue

        csv_files = sorted(glob.glob(os.path.join(grade_dir, "**", "*.csv"), recursive=True))
        print(f"  {grade}: {len(csv_files)} file")

        for fpath in csv_files:
            df = pd.read_csv(fpath)

            # Pastikan kolom sensor lengkap
            missing = [c for c in SENSOR_COLS if c not in df.columns]
            if missing:
                print(f"  SKIP (kolom kurang: {missing}): {fpath}")
                continue

            # Auto-detect kolom waktu: cari 'timestamp', atau pakai kolom pertama
            time_col = None
            if "timestamp" in df.columns:
                time_col = "timestamp"
            else:
                # Kolom pertama yang bukan sensor dianggap waktu
                non_sensor = [c for c in df.columns if c not in SENSOR_COLS]
                if non_sensor:
                    time_col = non_sensor[0]

            # Ambil hanya 300 baris
            df = df.head(300).copy()

            if time_col:
                df = df[[time_col] + SENSOR_COLS].copy()
                df.columns = ["time"] + SENSOR_COLS
            else:
                # Tidak ada kolom waktu sama sekali, buat sendiri
                df = df[SENSOR_COLS].copy()
                df.insert(0, "time", range(1, len(df) + 1))

            # Tambahkan kolom id untuk TSFRESH
            df.insert(0, "id", sample_id)

            records.append(df)
            label_map[sample_id] = label
            sample_meta.append({
                "id": sample_id,
                "grade": grade,
                "folder": os.path.basename(os.path.dirname(fpath)),
                "filename": os.path.basename(fpath),
            })
            sample_id += 1

    if not records:
        raise RuntimeError(f"Tidak ada file CSV ditemukan di {processed_dir}")

    df_tsfresh = pd.concat(records, ignore_index=True)
    labels = pd.Series(label_map, name="label")
    labels.index.name = "id"

    print(f"  Total sampel: {sample_id}")
    print(f"  High grade : {sum(1 for v in label_map.values() if v == 1)}")
    print(f"  Low grade  : {sum(1 for v in label_map.values() if v == 0)}")
    print(f"  Shape df   : {df_tsfresh.shape}")

    return df_tsfresh, labels, sample_meta


def extract_features(df_tsfresh: pd.DataFrame) -> pd.DataFrame:
    """Ekstraksi fitur TSFRESH dengan EfficientFCParameters."""
    from tsfresh import extract_features as tsfresh_extract
    from tsfresh.feature_extraction import EfficientFCParameters

    print("\n[2/5] Ekstraksi fitur TSFRESH (EfficientFCParameters) ...")
    print("      Ini mungkin memakan waktu 5–15 menit tergantung CPU ...")
    t0 = time.time()

    features = tsfresh_extract(
        df_tsfresh,
        column_id="id",
        column_sort="time",
        default_fc_parameters=EfficientFCParameters(),
        disable_progressbar=False,
        n_jobs=0,          # 0 = gunakan semua core
    )

    # Hapus kolom yang seluruhnya NaN
    features = features.dropna(axis=1, how="all")

    # Isi NaN sisa dengan 0
    features = features.fillna(0)

    elapsed = time.time() - t0
    print(f"  Selesai dalam {elapsed:.1f} detik")
    print(f"  Jumlah fitur raw: {features.shape[1]}")

    return features


def select_features(features: pd.DataFrame, labels: pd.Series) -> pd.DataFrame:
    """Seleksi fitur dengan FRESH algorithm (FDR Benjamini-Yekutieli)."""
    from tsfresh.feature_selection import select_features as tsfresh_select

    print(f"\n[3/5] Seleksi fitur FRESH (FDR level = {FDR_LEVEL}) ...")

    # Align index
    common_idx = features.index.intersection(labels.index)
    X = features.loc[common_idx]
    y = labels.loc[common_idx]

    t0 = time.time()
    X_selected = tsfresh_select(X, y, fdr_level=FDR_LEVEL)
    elapsed = time.time() - t0

    print(f"  Selesai dalam {elapsed:.1f} detik")
    print(f"  Fitur sebelum seleksi : {X.shape[1]}")
    print(f"  Fitur setelah FRESH   : {X_selected.shape[1]}")

    return X_selected


def prune_correlated(features: pd.DataFrame, threshold: float = 0.95) -> pd.DataFrame:
    """
    Hapus fitur yang berkorelasi tinggi (Pearson > threshold).
    Dari pasangan fitur berkorelasi, pertahankan yang pertama secara alfabetis.
    """
    print(f"\n[4/5] Pruning korelasi (threshold = {threshold}) ...")

    if features.shape[1] <= 1:
        return features

    corr_matrix = features.corr(method="pearson").abs()

    # Upper triangle mask
    upper = corr_matrix.where(
        np.triu(np.ones(corr_matrix.shape), k=1).astype(bool)
    )

    # Kolom yang perlu dihapus
    to_drop = [col for col in upper.columns if any(upper[col] > threshold)]
    features_pruned = features.drop(columns=to_drop)

    print(f"  Fitur setelah FRESH       : {features.shape[1]}")
    print(f"  Dihapus (korelasi tinggi) : {len(to_drop)}")
    print(f"  Fitur setelah pruning     : {features_pruned.shape[1]}")

    return features_pruned


def limit_top_n(features: pd.DataFrame, labels: pd.Series, top_n: int) -> pd.DataFrame:
    """
    Jika fitur masih terlalu banyak, pilih top-N berdasarkan
    skor mutual information terhadap label.
    """
    if features.shape[1] <= top_n:
        return features

    print(f"\n  Membatasi ke top {top_n} fitur (mutual information) ...")
    from sklearn.feature_selection import mutual_info_classif

    common_idx = features.index.intersection(labels.index)
    X = features.loc[common_idx]
    y = labels.loc[common_idx]

    mi_scores = mutual_info_classif(X, y, random_state=42)
    mi_series = pd.Series(mi_scores, index=X.columns)
    top_cols = mi_series.nlargest(top_n).index.tolist()

    print(f"  Dipilih {len(top_cols)} fitur teratas")
    return features[top_cols]


def save_outputs(
    features_raw,
    features_selected,
    features_final,
    labels,
    sample_meta,
    output_dir,
):
    """Simpan semua output ke data/features/."""
    print(f"\n[5/5] Menyimpan hasil ke {output_dir}/ ...")
    os.makedirs(output_dir, exist_ok=True)

    # 1. Raw features
    features_raw.to_csv(os.path.join(output_dir, "features_raw.csv"))
    print(f"  features_raw.csv       -> {features_raw.shape}")

    # 2. Selected features (setelah FRESH)
    features_selected.to_csv(os.path.join(output_dir, "features_selected.csv"))
    print(f"  features_selected.csv  -> {features_selected.shape}")

    # 3. Final features (setelah pruning + top-N) — INPUT MODEL RUST
    features_final.to_csv(os.path.join(output_dir, "features_final.csv"))
    print(f"  features_final.csv     -> {features_final.shape}  ← INPUT MODEL")

    # 4. Labels
    labels.to_csv(os.path.join(output_dir, "labels.csv"), header=True)
    print(f"  labels.csv             -> {len(labels)} sampel")

    # 4b. Groups (id -> origin) untuk Leave-One-Origin-Out (LOOO)
    #     origin = "{grade}/{folder}" mis. "high_grade/sample1"
    import csv as _csv
    groups_path = os.path.join(output_dir, "groups.csv")
    with open(groups_path, "w", newline="") as gf:
        writer = _csv.writer(gf)
        writer.writerow(["id", "origin", "grade", "folder"])
        for m in sample_meta:
            origin = f"{m['grade']}/{m['folder']}"
            writer.writerow([m["id"], origin, m["grade"], m["folder"]])
    n_origins = len({f"{m['grade']}/{m['folder']}" for m in sample_meta})
    print(f"  groups.csv             -> {len(sample_meta)} sampel, {n_origins} origin (untuk LOOO)")

    # 5. Nama fitur terpilih (dibaca oleh Rust)
    feature_names = features_final.columns.tolist()
    with open(os.path.join(output_dir, "selected_feature_names.json"), "w") as f:
        json.dump({
            "n_features": len(feature_names),
            "feature_names": feature_names,
            "sensors": SENSOR_COLS,
            "fdr_level": FDR_LEVEL,
            "corr_threshold": CORR_THRESHOLD,
        }, f, indent=2)
    print(f"  selected_feature_names.json -> {len(feature_names)} fitur")

    # 6. Laporan ringkasan
    report_lines = [
        "=" * 60,
        "LAPORAN TSFRESH PIPELINE",
        "=" * 60,
        f"Total sampel          : {len(labels)}",
        f"  High grade (1)      : {(labels == 1).sum()}",
        f"  Low grade  (0)      : {(labels == 0).sum()}",
        f"Sensor                : {', '.join(SENSOR_COLS)}",
        f"Timestep per sampel   : 300",
        "",
        "Hasil Seleksi Fitur:",
        f"  Raw (sebelum seleksi)   : {features_raw.shape[1]}",
        f"  Setelah FRESH + FDR     : {features_selected.shape[1]}",
        f"  Setelah pruning korelasi: {features_final.shape[1]}",
        "",
        "Parameter:",
        f"  FDR level              : {FDR_LEVEL}",
        f"  Korelasi threshold     : {CORR_THRESHOLD}",
        f"  Top-N maksimal         : {TOP_N_FEATURES}",
        "",
        "Top 20 Fitur Final (input model):",
    ]
    for i, name in enumerate(feature_names[:20], 1):
        report_lines.append(f"  {i:2d}. {name}")
    if len(feature_names) > 20:
        report_lines.append(f"  ... dan {len(feature_names) - 20} fitur lainnya")

    report_path = os.path.join(output_dir, "feature_report.txt")
    with open(report_path, "w") as f:
        f.write("\n".join(report_lines))
    print(f"  feature_report.txt")

    print(f"\n{'=' * 60}")
    print(f"  PIPELINE SELESAI")
    print(f"  {features_final.shape[1]} fitur final siap digunakan model Rust")
    print(f"  File input model: {output_dir}/features_final.csv")
    print(f"{'=' * 60}\n")


def main():
    print("=" * 60)
    print("  TSFRESH PIPELINE — E-Nose Kopi Arabica")
    print("  High Grade vs Low Grade Classification")
    print("=" * 60)

    # Pastikan dijalankan dari root project
    if not os.path.exists(PROCESSED_DIR):
        raise RuntimeError(
            f"Folder '{PROCESSED_DIR}' tidak ditemukan.\n"
            f"Pastikan script dijalankan dari root folder MachineLearningModel/\n"
            f"Contoh: cd MachineLearningModel && python tools/python/tsfresh_pipeline.py"
        )

    # 1. Load data
    df_tsfresh, labels, sample_meta = load_all_samples(PROCESSED_DIR)

    # 2. Ekstraksi fitur
    features_raw = extract_features(df_tsfresh)

    # 3. Seleksi FRESH + FDR
    features_selected = select_features(features_raw, labels)

    # 4. Pruning korelasi
    features_pruned = prune_correlated(features_selected, threshold=CORR_THRESHOLD)

    # 5. Limit top-N jika masih terlalu banyak
    features_final = limit_top_n(features_pruned, labels, top_n=TOP_N_FEATURES)

    # 6. Simpan semua output
    save_outputs(
        features_raw=features_raw,
        features_selected=features_selected,
        features_final=features_final,
        labels=labels,
        sample_meta=sample_meta,
        output_dir=FEATURES_DIR,
    )


if __name__ == "__main__":
    main()