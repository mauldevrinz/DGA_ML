"""Ekstraksi 37 fitur tsfresh untuk model SNN baru (identik dengan training).
Input : DataFrame/array rekaman time-series, kolom MOS0..MOS15 (mV), 1 baris = 1 sampel waktu,
        urutan baris = urutan waktu. Panjang & laju sampling HARUS sama dengan rekaman training.
Output: np.float32 vektor panjang 37 (urutan = feature_names), NaN/Inf sudah diimputasi."""
import json, numpy as np, pandas as pd
from tsfresh import extract_features
from tsfresh.feature_extraction.settings import from_columns

_meta = None
def _load(meta_path="feature_meta.json"):
    global _meta
    if _meta is None:
        _meta = json.load(open(meta_path))
        _meta["fc"] = from_columns(_meta["feature_names"])      # hanya 37 fitur, bukan semua EfficientFC
    return _meta

def extract_37(rec, meta_path="feature_meta.json"):
    m = _load(meta_path)
    df = pd.DataFrame(rec) if not isinstance(rec, pd.DataFrame) else rec.copy()
    df.columns = [f"MOS{i}" for i in range(df.shape[1])] if df.shape[1] == 16 and not str(df.columns[0]).startswith("MOS") else df.columns
    df = df.apply(pd.to_numeric, errors="coerce").reset_index(drop=True).reset_index().rename(columns={"index": "time"})
    df["id"] = 0
    long = df.melt(id_vars=["id", "time"], var_name="channel", value_name="value").dropna()
    long = long[long["channel"].isin(m["fc"].keys())]           # hanya kanal yang dipakai
    feats = extract_features(long, column_id="id", column_sort="time", column_kind="channel",
                             column_value="value", kind_to_fc_parameters=m["fc"],
                             n_jobs=0, disable_progressbar=True)
    x = feats.reindex(columns=m["feature_names"]).replace([np.inf, -np.inf], np.nan).iloc[0].to_numpy(dtype=np.float64)
    imp = np.asarray(m["impute"], dtype=np.float64)
    x = np.where(np.isfinite(x), x, imp)
    return x.astype(np.float32)
