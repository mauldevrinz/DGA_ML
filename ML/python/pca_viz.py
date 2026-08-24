#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
PCA visualization of RAW e-nose data for Arabica coffee.

PIPELINE: raw -> z-score (StandardScaler) -> PCA
NO baseline correction.

Methodology note (important for publication):
    This pipeline is mathematically correct and is standard PCA. However,
    because baseline correction is NOT applied, separation between points may
    partly come from sensor drift/offset across measurement sessions, NOT
    purely from coffee aroma differences. Interpret separation with caution.

Each raw measurement in data/raw/<grade>/<sample>/*.csv (8 MOS sensors) is
converted into one feature vector: the mean raw value of each sensor over time
(8-dim). The whole dataset is then z-scored and projected with PCA.

- Each point = 1 measurement.
- Color = origin. 6 base hues, each as a light/dark pair:
  HIGH grade = light shade, LOW grade = dark shade of the same hue.
- Legend label = "H-<Origin>" for high, "L-<Origin>" for low.

On run: a matplotlib window opens AND the figure is saved to models/pca_raw.png.

Usage:
    python pca_viz.py [--data-root .] [--no-show]
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
from sklearn.decomposition import PCA

SENSORS = ["tgs2600", "mq135", "mq3", "mq6", "mq7", "tgs2602", "tgs2611", "tgs2620"]

# ============================================================================
# ORIGIN NAME MAPPING
# data/raw/ uses generic folder names (sample1..6). Map them to real origin
# names here. Key = "<grade>/<folder>" (lowercase). EDIT to match your data.
# If a folder is missing here, the folder name is used as-is.
# ============================================================================
SAMPLE_NAMES = {
    "high_grade/sample1": "Papua Wamena",
    "high_grade/sample2": "Bali Kintamani",
    "high_grade/sample3": "Central Java",
    "high_grade/sample4": "Ijen",
    "high_grade/sample5": "Gayo",
    "high_grade/sample6": "Toraja",
    "low_grade/sample1":  "Situbondo",
    "low_grade/sample2":  "Kerinci",
    "low_grade/sample3":  "Gayo",
    "low_grade/sample4":  "Flores",
    "low_grade/sample5":  "East Java",
    "low_grade/sample6":  "Jember",
}

# ============================================================================
# COLOR SYSTEM: 6 base hues, each a (light, dark) pair.
# HIGH grade -> light shade, LOW grade -> dark shade of the SAME hue.
# Pairing is by sample index: high_grade/sampleN shares a hue with
# low_grade/sampleN. EDIT the pairs below if you want different hue groupings.
# ============================================================================
HUE_PAIRS = {
    "sample1": ("#E53935", "#7F0000"),  # red:    bright red    / dark red
    "sample2": ("#FB8C00", "#E65100"),  # orange: bright orange / burnt orange
    "sample3": ("#43A047", "#1B5E20"),  # green:  bright green  / forest green
    "sample4": ("#1E88E5", "#0D47A1"),  # blue:   bright blue   / navy
    "sample5": ("#8E24AA", "#4A148C"),  # purple: bright purple / dark purple
    "sample6": ("#00ACC1", "#006064"),  # cyan:   bright cyan   / dark teal
}
FALLBACK_HIGH = "#90A4AE"
FALLBACK_LOW = "#37474F"


def raw_response(df):
    """Mean RAW value of each sensor over time (no baseline).

    Returns an 8-dim vector (one value per sensor)."""
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
    """origin = '<grade>/<sample>', grade, sample from folder structure."""
    rel = os.path.relpath(path, os.path.join(data_root, "data", "raw"))
    parts = rel.replace("\\", "/").split("/")
    if len(parts) >= 2:
        return f"{parts[0]}/{parts[1]}", parts[0], parts[1]
    return "unknown", "unknown", "unknown"


def resolve_origin(grade, sample):
    """Map generic folder -> real origin name via SAMPLE_NAMES."""
    key = f"{grade.lower()}/{sample.lower()}"
    if key in SAMPLE_NAMES:
        return SAMPLE_NAMES[key]
    name = sample.replace("_", " ").strip()
    return " ".join(w.capitalize() for w in name.split())


def make_label(grade, sample):
    """Legend format singkat: inisial tiap kata origin. Mis. 'Papua Wamena' -> 'H-PW',
    'Situbondo' -> 'L-S', 'Central Java' -> 'H-CJ'."""
    prefix = "H" if grade.lower().startswith("high") else "L"
    origin = resolve_origin(grade, sample)
    initials = "".join(w[0].upper() for w in origin.split() if w)
    return f"{prefix}-{initials}"


def color_for(grade, sample):
    """Light shade for high grade, dark shade for low grade, paired by hue."""
    is_high = grade.lower().startswith("high")
    key = sample.lower()
    if key in HUE_PAIRS:
        light, dark = HUE_PAIRS[key]
    else:
        light, dark = FALLBACK_HIGH, FALLBACK_LOW
    return light if is_high else dark


def load_raw(data_root):
    raw_dir = os.path.join(data_root, "data", "raw")
    files = sorted(glob.glob(os.path.join(raw_dir, "*", "*", "*.csv")))
    X, labels, grades, samples = [], [], [], []
    for f in files:
        try:
            df = pd.read_csv(f)
        except Exception:
            continue
        X.append(raw_response(df))
        _, grd, smp = origin_from_path(f, data_root)
        grades.append(grd)
        samples.append(smp)
        labels.append(make_label(grd, smp))
    return np.array(X, dtype=float), labels, grades, samples


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data-root", default=".", help="root containing the data/ folder")
    ap.add_argument("--no-show", action="store_true", help="only save PNG, do not open a window")
    args = ap.parse_args()

    X, labels, grades, samples = load_raw(args.data_root)
    if X.shape[0] == 0:
        print("[X] No raw data found at data/raw/*/*/*.csv")
        sys.exit(1)
    print(f"[OK] Loaded {X.shape[0]} measurements x {X.shape[1]} sensors.")

    # PIPELINE: raw -> z-score -> PCA
    Xs = StandardScaler().fit_transform(X)
    pca = PCA(n_components=2)
    Z = pca.fit_transform(Xs)
    ev = pca.explained_variance_ratio_ * 100.0

    # Legend order: all High first, then all Low.
    uniq = sorted(set(labels),
                  key=lambda lb: (0 if lb.startswith("H-") else 1, lb))

    if args.no_show:
        matplotlib.use("Agg")

    fig, ax = plt.subplots(figsize=(11, 7))
    for lb in uniq:
        idx = [i for i, l in enumerate(labels) if l == lb]
        g = grades[idx[0]]
        s = samples[idx[0]]
        ax.scatter(Z[idx, 0], Z[idx, 1], s=55, c=color_for(g, s), label=lb,
                   edgecolors="black", linewidths=0.4, alpha=0.85)
    ax.set_xlabel(f"PC1 ({ev[0]:.1f}%)", fontsize=12, fontweight="bold")
    ax.set_ylabel(f"PC2 ({ev[1]:.1f}%)", fontsize=12, fontweight="bold")
    ax.set_title("PCA - E-Nose Arabica Coffee",
                 fontsize=14, fontweight="bold")
    ax.grid(True, linestyle="--", alpha=0.4)
    ax.legend(title="Origin", fontsize=8, ncol=1, loc="center left",
              bbox_to_anchor=(1.01, 0.5), framealpha=0.9)
    fig.tight_layout()

    out_dir = os.path.join(args.data_root, "models")
    os.makedirs(out_dir, exist_ok=True)
    out_png = os.path.join(out_dir, "pca_raw.png")
    fig.savefig(out_png, dpi=150, bbox_inches="tight")
    print(f"[OK] PNG saved: {out_png}")

    if not args.no_show:
        plt.show()


if __name__ == "__main__":
    main()