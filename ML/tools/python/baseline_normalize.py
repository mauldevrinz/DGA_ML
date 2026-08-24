import pandas as pd
import os
import glob

# ── Konfigurasi ──────────────────────────────────────────────────────────────
INPUT_DIR  = "../../data/raw"       # relatif dari lokasi script ini
OUTPUT_DIR = "../../data/processed"
# ─────────────────────────────────────────────────────────────────────────────


def normalize_file(input_path: str, output_path: str) -> None:
    df = pd.read_csv(input_path)

    sensor_cols = [c for c in df.columns if c != "timestamp"]

    # 1. Subtract baseline
    df[sensor_cols] = df[sensor_cols] - df[sensor_cols].iloc[0]

    # 2. Clip negatif → 0
    df[sensor_cols] = df[sensor_cols].clip(lower=0)

    # 3. Round 2 desimal
    df[sensor_cols] = df[sensor_cols].round(2)

    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    df.to_csv(output_path, index=False)


def main():
    csv_files = glob.glob(os.path.join(INPUT_DIR, "**", "*.csv"), recursive=True)

    if not csv_files:
        print(f"Tidak ada file CSV ditemukan di: {INPUT_DIR}")
        return

    print(f"Ditemukan {len(csv_files)} file CSV, mulai proses...\n")

    for input_path in sorted(csv_files):
        # Bangun output path dengan struktur folder yang sama
        relative = os.path.relpath(input_path, INPUT_DIR)
        output_path = os.path.join(OUTPUT_DIR, relative)

        normalize_file(input_path, output_path)
        print(f"[OK] {relative}")

    print(f"\nSelesai! {len(csv_files)} file disimpan ke: {OUTPUT_DIR}")


if __name__ == "__main__":
    main()