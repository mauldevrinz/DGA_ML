"""
tsfresh_predict_single.py
--------------------------
Ekstrak fitur TSFRESH dari SATU file CSV untuk prediksi.
Fitur dihitung DENGAN CARA SAMA PERSIS seperti training, lalu hanya
fitur yang ada di selected_feature_names.json yang diambil (urutan dijaga).

Output: JSON ke stdout berisi { "features": [...], "feature_names": [...] }
        sehingga bisa dibaca oleh Rust.

Cara pakai (dipanggil otomatis oleh predict GUI Rust):
    python tools/python/tsfresh_predict_single.py <path_ke_csv>

Contoh manual:
    python tools/python/tsfresh_predict_single.py data/test/sampel_baru.csv

Catatan: butuh data/features/selected_feature_names.json (hasil training).
"""

import sys
import json
import warnings
import numpy as np
import pandas as pd

warnings.filterwarnings("ignore")

SENSOR_COLS = ["tgs2600", "mq135", "mq3", "mq6", "mq7", "tgs2602", "tgs2611", "tgs2620"]
FEATURES_DIR = "data/features"


def eprint(*args):
    """Print ke stderr supaya tidak mengganggu output JSON di stdout."""
    print(*args, file=sys.stderr)


def load_selected_features():
    """Baca nama fitur terpilih dari training."""
    path = f"{FEATURES_DIR}/selected_feature_names.json"
    with open(path, "r") as f:
        meta = json.load(f)
    return meta["feature_names"]


def detect_time_col(df):
    if "timestamp" in df.columns:
        return "timestamp"
    non_sensor = [c for c in df.columns if c not in SENSOR_COLS]
    return non_sensor[0] if non_sensor else None


def extract_features_single(csv_path, selected_names):
    from tsfresh import extract_features
    from tsfresh.feature_extraction import EfficientFCParameters

    df = pd.read_csv(csv_path)

    missing = [c for c in SENSOR_COLS if c not in df.columns]
    if missing:
        raise ValueError(f"Kolom sensor hilang: {missing}")

    # ── BASELINE NORMALIZATION (WAJIB — samakan dengan baseline_normalize.py) ──
    # Training memakai data/processed yang SUDAH di-baseline-normalize.
    # Tanpa langkah ini, fitur prediksi dari CSV mentah tidak cocok dengan
    # fitur training → model salah prediksi (mis. "high semua").
    #   1. Subtract nilai baris pertama tiap sensor
    #   2. Clip negatif → 0
    #   3. Round 2 desimal
    df[SENSOR_COLS] = df[SENSOR_COLS] - df[SENSOR_COLS].iloc[0]
    df[SENSOR_COLS] = df[SENSOR_COLS].clip(lower=0)
    df[SENSOR_COLS] = df[SENSOR_COLS].round(2)

    df = df.head(300).copy()
    time_col = detect_time_col(df)
    if time_col:
        df = df[[time_col] + SENSOR_COLS].copy()
        df.columns = ["time"] + SENSOR_COLS
    else:
        df = df[SENSOR_COLS].copy()
        df.insert(0, "time", range(1, len(df) + 1))

    df.insert(0, "id", 0)  # satu sampel → id=0

    # Ekstrak SEMUA fitur (sama seperti training), lalu pilih yang relevan
    all_feats = extract_features(
        df,
        column_id="id",
        column_sort="time",
        default_fc_parameters=EfficientFCParameters(),
        disable_progressbar=True,
        n_jobs=0,
    )
    all_feats = all_feats.fillna(0)

    # Ambil hanya fitur terpilih, urutan sama dengan training
    values = []
    for name in selected_names:
        if name in all_feats.columns:
            values.append(float(all_feats.iloc[0][name]))
        else:
            # Jika fitur tidak terhitung (mis. konstan), isi 0
            values.append(0.0)

    return values


def main():
    if len(sys.argv) < 2:
        eprint("Usage: python tsfresh_predict_single.py <path_ke_csv>")
        print(json.dumps({"error": "no csv path provided"}))
        sys.exit(1)

    csv_path = sys.argv[1]

    try:
        selected = load_selected_features()
        eprint(f"[tsfresh_predict] {len(selected)} fitur terpilih dimuat")

        values = extract_features_single(csv_path, selected)
        eprint(f"[tsfresh_predict] {len(values)} fitur diekstrak dari {csv_path}")

        # Output JSON ke stdout (HANYA ini di stdout, agar Rust mudah parse)
        print(json.dumps({
            "features": values,
            "feature_names": selected,
        }))
    except Exception as e:
        eprint(f"[tsfresh_predict] ERROR: {e}")
        print(json.dumps({"error": str(e)}))
        sys.exit(1)


if __name__ == "__main__":
    main()