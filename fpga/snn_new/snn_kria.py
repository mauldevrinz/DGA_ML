"""Driver SNN baru untuk KV260 (menggantikan fpga/snn_driver.py + params.bin lama).
Alur: rekaman MOS0..MOS15 (mV) -> 37 fitur tsfresh -> akselerator FPGA (snn.bit) -> 5 logit -> confidence.
Mode HW  : butuh PYNQ (Kria-PYNQ) + snn.bit/snn.hwh di folder ini.
Mode SW  : referensi numpy (snn_weights.npz) tanpa FPGA, untuk membandingkan hasil HW vs SW.
Kelas keluaran (urutan model): C2H2, C2H4, CH4, H2, OTHER."""
import os, time, numpy as np
from dga_features import extract_37

DIR = os.path.dirname(os.path.abspath(__file__))
GAS = ["C2H2", "C2H4", "CH4", "H2", "OTHER"]
_W = np.load(os.path.join(DIR, "snn_weights.npz"))
_hw = {}

def init_hw():
    """Panggil SEKALI saat startup (sebelum fork/thread): load bitstream + alokasi buffer."""
    if _hw: return
    from pynq import Overlay, allocate
    ov = Overlay(os.path.join(DIR, "snn.bit"))
    _hw.update(ov=ov, ip=ov.snn_top_0, x=allocate(37, np.float32), lg=allocate(5, np.float32))

def _logits_hw(f):
    init_hw()
    ip, xb, lb = _hw["ip"], _hw["x"], _hw["lg"]
    xb[:] = f; xb.flush()
    rm = ip.register_map
    rm.x_raw_1 = xb.physical_address & 0xFFFFFFFF;  rm.x_raw_2 = xb.physical_address >> 32
    rm.logits_1 = lb.physical_address & 0xFFFFFFFF; rm.logits_2 = lb.physical_address >> 32
    ip.write(0x00, 1)                              # ap_start
    t0 = time.monotonic()
    while not (ip.read(0x00) >> 1) & 1:            # tunggu ap_done
        if time.monotonic() - t0 > 2.0: raise TimeoutError("akselerator tidak selesai (2 s)")
    lb.invalidate()
    return np.array(lb, dtype=np.float32)

def _logits_sw(f):
    z = (_W["W1"] @ ((f - _W["mean"]) * _W["inv_scale"]) + _W["B1"]) > 1.0   # LIF == threshold biner
    h = z.reshape(5, 24).astype(np.float32)
    return (h * _W["W2"]).sum(1) + _W["B2"]

def snn_infer(recording, use_hw=True):
    """recording: array/DataFrame [N,16] mV, atau list of dict {'mos0':..,'mos15':..}."""
    try:
        if isinstance(recording, list) and recording and isinstance(recording[0], dict):
            recording = [[p.get(f"mos{i}", np.nan) for i in range(16)] for p in recording]
        t0 = time.monotonic()
        f = extract_37(recording)
        t1 = time.monotonic()
        lg = _logits_hw(f) if use_hw else _logits_sw(f)
        t2 = time.monotonic()
        conf = 1.0 / (1.0 + np.exp(-lg.astype(np.float64)))           # sigmoid, sama dgn predict_proba
        k = int(np.argmax(lg))
        return {"ok": True, "gas_names": GAS, "outputs": [float(c) for c in conf], "logits": [float(v) for v in lg],
                "gas_confidences": {g: round(float(c) * 100, 1) for g, c in zip(GAS, conf)},
                "predicted_gas": GAS[k], "predicted_idx": k, "confidence": float(conf[k]),
                "feature_ms": round((t1 - t0) * 1000, 1), "latency_ms": round((t2 - t1) * 1000, 3)}
    except Exception as e:
        import traceback
        return {"ok": False, "error": str(e), "trace": traceback.format_exc()}
