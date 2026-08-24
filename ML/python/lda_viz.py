#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Visualisasi LDA dari DATA MENTAH e-nose Arabica coffee.

PIPELINE: raw -> z-score (StandardScaler) -> LDA
TANPA baseline correction (sama seperti pca_viz.py).

Proyeksi dihitung dengan Linear Discriminant Analysis (LDA) yang TERSUPERVISI
memakai label kelas high/low. Karena kelas hanya dua, LDA menghasilkan 1
komponen diskriminan (LD1). Agar dapat divisualisasikan 2D, sumbu-Y memakai PC1
(ortogonal, tak tersupervisi) sebagai konteks sebaran. LD1 = sumbu pemisah kelas.

Label & warna: HANYA dua kelas, high_grade vs low_grade.

Catatan metodologi (penting untuk publikasi):
    Pipeline raw -> z-score (tanpa baseline) matematis benar, namun separasi
    bisa sebagian berasal dari drift/offset sensor antar-sesi pengukuran, BUKAN
    murni perbedaan aroma kopi. Tafsirkan dengan hati-hati.

Saat dijalankan: jendela matplotlib terbuka DAN gambar otomatis disimpan ke
models/lda_raw.png.

Pemakaian:
    python lda_viz.py [--data-root .] [--no-show]
"""
import os
import sys
import argparse
import glob
import numpy as np
import pandas as pd
import matplotlib
import matplotlib.pyplot as plt
from sklearn.preprocessing import StandardScaler
from sklearn.discriminant_analysis import LinearDiscriminantAnalysis
from sklearn.decomposition import PCA

SENSORS = ["tgs2600", "mq135", "mq3", "mq6", "mq7", "tgs2602", "tgs2611", "tgs2620"]

GRADE_COLORS = {"high_grade": "#2E7D32", "low_grade": "#C62828"}


def raw_response(df):
    """Rata-rata nilai MENTAH tiap sensor sepanjang waktu (tanpa baseline).

    Mengembalikan vektor 8-dim (satu nilai per sensor)."""
    feats = []
    for s in SENSORS:
        if s not in df.columns:
            feats.append(0.0)
            continue
        v = pd.to_numeric(df[s], errors="coerce").to_numpy(dtype=float)
        v = v[np.isfinite(v)]
        if v.size == 0:
            feats.append(0.0)
            continue
        feats.append(float(np.mean(v)))
    return feats


def origin_from_path(path, data_root):
    rel = os.path.relpath(path, os.path.join(data_root, "data", "raw"))
    parts = rel.replace("\\", "/").split("/")
    if len(parts) >= 2:
        return f"{parts[0]}/{parts[1]}", parts[0]
    return "unknown", "unknown"


def load_raw(data_root):
    raw_dir = os.path.join(data_root, "data", "raw")
    files = sorted(glob.glob(os.path.join(raw_dir, "*", "*", "*.csv")))
    X, origins, grades = [], [], []
    for f in files:
        try:
            df = pd.read_csv(f)
        except Exception:
            continue
        X.append(raw_response(df))
        org, grd = origin_from_path(f, data_root)
        origins.append(org)
        grades.append(grd)
    return np.array(X, dtype=float), origins, grades


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data-root", default=".")
    ap.add_argument("--no-show", action="store_true")
    args = ap.parse_args()

    X, origins, grades = load_raw(args.data_root)
    if X.shape[0] == 0:
        print("[X] Tidak ada data mentah ditemukan di data/raw/*/*/*.csv")
        sys.exit(1)
    print(f"[OK] {X.shape[0]} pengukuran x {X.shape[1]} sensor dimuat.")

    # PIPELINE: raw -> z-score -> LDA
    y = np.array([1 if g == "high_grade" else 0 for g in grades])
    Xs = StandardScaler().fit_transform(X)

    # LD1: sumbu diskriminan tersupervisi (high vs low)
    lda = LinearDiscriminantAnalysis(n_components=1)
    ld1 = lda.fit_transform(Xs, y)[:, 0]

    # PC1 sebagai sumbu konteks (ortogonal, tak tersupervisi)
    pc1 = PCA(n_components=1).fit_transform(Xs)[:, 0]

    if args.no_show:
        matplotlib.use("Agg")

    fig, ax = plt.subplots(figsize=(10, 7))
    for grd in ["high_grade", "low_grade"]:
        idx = [i for i, gg in enumerate(grades) if gg == grd]
        ax.scatter(ld1[idx], pc1[idx], s=55, c=GRADE_COLORS[grd],
                   label=grd, edgecolors="black", linewidths=0.4, alpha=0.85)
    ax.set_xlabel("LD1 (linear discriminant)", fontsize=12, fontweight="bold")
    ax.set_ylabel("PC1 (context axis)", fontsize=12, fontweight="bold")
    ax.set_title("LDA - E-Nose Arabica Coffee",
                 fontsize=14, fontweight="bold")
    ax.grid(True, linestyle="--", alpha=0.4)
    ax.legend(title="Grade", fontsize=10, loc="best", framealpha=0.9)
    fig.tight_layout()

    out_dir = os.path.join(args.data_root, "models")
    os.makedirs(out_dir, exist_ok=True)
    out_png = os.path.join(out_dir, "lda_raw.png")
    fig.savefig(out_png, dpi=150)
    print(f"[OK] PNG disimpan: {out_png}")

    if not args.no_show:
        plt.show()


if __name__ == "__main__":
    main()