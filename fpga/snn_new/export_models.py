#!/usr/bin/env python3
"""Export gas_model.joblib, fault_model.joblib, snn_weights.npz ke JSON
yang bisa dibaca oleh Rust binary snn_infer.

Jalankan SEKALI setelah training:
  python3 fpga/snn_new/export_models.py

Output (di fpga/snn_infer/models/):
  snn_weights.json   — bobot SNN + impute dari feature_meta.json
  fault_trees.json   — pohon Random Forest untuk klasifikasi fault
"""
import json, sys
from pathlib import Path

DIR   = Path(__file__).parent
OUT   = DIR.parent / "snn_infer" / "models"
OUT.mkdir(parents=True, exist_ok=True)

# ── 1. SNN weights ──────────────────────────────────────────────────────────
import numpy as np
print("Mengekspor snn_weights.npz …")
w = np.load(DIR / "snn_weights.npz")
meta = json.loads((DIR / "feature_meta.json").read_text())

snn_json = {
    "w1":       w["W1"].tolist(),       # [[37]; 120]
    "b1":       w["B1"].tolist(),       # [120]
    "w2":       w["W2"].tolist(),       # [[24]; 5]
    "b2":       w["B2"].tolist(),       # [5]
    "mean":     w["mean"].tolist(),     # [37]
    "inv_scale": w["inv_scale"].tolist(), # [37]
    "impute":   meta["impute"],         # [37]
    "gas_classes": meta["gas_classes"], # ["C2H2","C2H4","CH4","H2","OTHER"]
}
(OUT / "snn_weights.json").write_text(json.dumps(snn_json, indent=2))
print(f"  → {OUT / 'snn_weights.json'}  ({len(meta['impute'])} fitur)")

# ── 2. Fault RF trees ────────────────────────────────────────────────────────
import joblib
print("Mengekspor fault_model.joblib …")
blob = joblib.load(DIR / "fault_model.joblib")
rf   = blob["model"]           # RandomForestClassifier
fault_classes = list(blob["fault_classes"])
gas_classes   = list(blob["gas_classes"])

trees = []
for est in rf.estimators_:
    t = est.tree_
    trees.append({
        "children_left":  t.children_left.tolist(),
        "children_right": t.children_right.tolist(),
        "feature":        t.feature.tolist(),
        "threshold":      t.threshold.tolist(),
        # value shape: [n_nodes, n_outputs, n_classes]
        "value": t.value[:, 0, :].tolist(),  # flatten n_outputs=1
    })

fault_json = {
    "trees":       trees,
    "classes":     fault_classes,   # ["T1","D1","N"]
    "n_features":  rf.n_features_in_,
    "gas_classes": gas_classes,
}
(OUT / "fault_trees.json").write_text(json.dumps(fault_json))
print(f"  → {OUT / 'fault_trees.json'}  ({len(trees)} pohon, {len(fault_classes)} kelas)")

print("\nEkspor selesai. Salin folder models/ sejajar dengan binary snn_infer.")
