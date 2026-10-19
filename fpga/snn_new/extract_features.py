#!/usr/bin/env python3
"""Extract the exact 37 tsfresh features expected by the trained SNN."""

import json
import os
import sys
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from dga_features import extract_37, _load

_META_PATH = os.path.join(HERE, "feature_meta.json")


def extract_request(request):
    recording = request.get("recording")
    if not isinstance(recording, list) or len(recording) < 10:
        raise ValueError("Recording must contain at least 10 sensor rows")
    if any(not isinstance(row, list) or len(row) != 16 for row in recording):
        raise ValueError("Each recording row must contain exactly 16 MOS channels")

    started = time.monotonic()
    features = extract_37(recording, meta_path=_META_PATH)
    elapsed_ms = (time.monotonic() - started) * 1000.0
    values = [float(value) for value in features]
    if len(values) != 37:
        raise ValueError(f"Expected 37 features, received {len(values)}")
    return {"ok": True, "features": values, "feature_ms": round(elapsed_ms, 3)}


def main():
    # Warm tsfresh configuration and imports before announcing readiness.
    _load(_META_PATH)
    print(json.dumps({"ready": True}), flush=True)
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            result = extract_request(json.loads(line))
        except Exception as exc:
            result = {"ok": False, "error": str(exc), "trace": traceback.format_exc(limit=3)}
        try:
            print(json.dumps(result, allow_nan=False), flush=True)
        except (TypeError, ValueError) as exc:
            print(json.dumps({"ok": False, "error": f"Could not encode extracted features: {exc}"}), flush=True)


if __name__ == "__main__":
    main()
