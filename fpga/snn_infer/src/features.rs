//! Reimplementasi 37 fitur tsfresh yang digunakan model SNN DGA.
//! Setiap fungsi menerima slice &[f64] dan mengembalikan f64 (NaN jika tidak valid).
//! Urutan output compute_all_features() HARUS sesuai feature_meta.json.

use std::collections::HashMap;
use std::f64::consts::PI;
use rustfft::{FftPlanner, num_complex::Complex};

// ─── Helper statistik ───────────────────────────────────────────────────────

#[inline]
fn mean(x: &[f64]) -> f64 {
    if x.is_empty() { return f64::NAN; }
    x.iter().sum::<f64>() / x.len() as f64
}

/// Variansi populasi (ddof=0), sama dengan np.var default.
#[inline]
fn pop_var(x: &[f64]) -> f64 {
    if x.len() < 2 { return f64::NAN; }
    let m = mean(x);
    x.iter().map(|&v| (v - m).powi(2)).sum::<f64>() / x.len() as f64
}

/// Quantile interpolasi linear (identik dengan numpy default).
fn quantile_of(sorted: &[f64], q: f64) -> f64 {
    let n = sorted.len();
    if n == 0 { return f64::NAN; }
    if n == 1 { return sorted[0]; }
    let idx = q * (n - 1) as f64;
    let lo  = idx.floor() as usize;
    let hi  = idx.ceil()  as usize;
    if lo == hi { return sorted[lo]; }
    sorted[lo] + (idx - lo as f64) * (sorted[hi] - sorted[lo])
}

fn sorted_copy(x: &[f64]) -> Vec<f64> {
    let mut s = x.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    s
}

/// Regresi linear y = slope*t + intercept, t = 0..n-1.
/// Mengembalikan (slope, intercept, r_value, stderr_slope).
fn linreg(y: &[f64]) -> (f64, f64, f64, f64) {
    let n = y.len();
    if n < 2 { return (f64::NAN, f64::NAN, f64::NAN, f64::NAN); }
    let nf = n as f64;
    // sum x = n(n-1)/2,  sum x² = n(n-1)(2n-1)/6
    let sx  = nf * (nf - 1.0) / 2.0;
    let sxx = nf * (nf - 1.0) * (2.0 * nf - 1.0) / 6.0;
    let sy: f64  = y.iter().sum();
    let sxy: f64 = y.iter().enumerate().map(|(i, &yi)| i as f64 * yi).sum();
    let syy: f64 = y.iter().map(|&yi| yi * yi).sum();
    let denom = nf * sxx - sx * sx;
    if denom.abs() < 1e-14 {
        return (0.0, sy / nf, 0.0, 0.0);
    }
    let slope     = (nf * sxy - sx * sy) / denom;
    let intercept = (sy - slope * sx) / nf;
    // r
    let num_r = nf * sxy - sx * sy;
    let den_r = (denom * (nf * syy - sy * sy)).sqrt();
    let rvalue = if den_r.abs() < 1e-14 { 0.0 } else { (num_r / den_r).clamp(-1.0, 1.0) };
    // stderr
    let sse: f64 = y.iter().enumerate()
        .map(|(i, &yi)| (yi - (slope * i as f64 + intercept)).powi(2))
        .sum();
    let mse  = sse / (nf - 2.0).max(1.0);
    let ssx  = sxx - sx * sx / nf;
    let stderr = if ssx.abs() < 1e-14 { 0.0 } else { (mse / ssx).sqrt() };
    (slope, intercept, rvalue, stderr)
}

fn linreg_attr(y: &[f64], attr: &str) -> f64 {
    let (slope, intercept, rvalue, stderr) = linreg(y);
    match attr {
        "slope"     => slope,
        "intercept" => intercept,
        "rvalue"    => rvalue,
        "stderr"    => stderr,
        _           => f64::NAN,
    }
}

/// Eliminasi Gauss untuk sistem Ax = b ukuran n×n.
fn gauss_elim(a: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let n = a.len();
    let mut mat: Vec<Vec<f64>> = (0..n)
        .map(|i| { let mut r = a[i].clone(); r.push(b[i]); r })
        .collect();
    for col in 0..n {
        let pivot_row = (col..n)
            .max_by(|&i, &j| mat[i][col].abs().partial_cmp(&mat[j][col].abs())
                              .unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(col);
        mat.swap(col, pivot_row);
        let pivot = mat[col][col];
        if pivot.abs() < 1e-12 { return vec![f64::NAN; n]; }
        for row in (col + 1)..n {
            let f = mat[row][col] / pivot;
            for j in col..=n { let sub = f * mat[col][j]; mat[row][j] -= sub; }
        }
    }
    let mut x = vec![0.0f64; n];
    for i in (0..n).rev() {
        x[i] = mat[i][n];
        for j in (i + 1)..n { x[i] -= mat[i][j] * x[j]; }
        x[i] /= mat[i][i];
    }
    x
}

// ─── Fitur-fitur individu ────────────────────────────────────────────────────

pub fn root_mean_square(x: &[f64]) -> f64 {
    if x.is_empty() { return f64::NAN; }
    (x.iter().map(|&v| v * v).sum::<f64>() / x.len() as f64).sqrt()
}

pub fn absolute_maximum(x: &[f64]) -> f64 {
    x.iter().map(|&v| v.abs()).fold(f64::NEG_INFINITY, f64::max)
}

pub fn maximum(x: &[f64]) -> f64 {
    x.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
}

pub fn minimum(x: &[f64]) -> f64 {
    x.iter().cloned().fold(f64::INFINITY, f64::min)
}

pub fn quantile(x: &[f64], q: f64) -> f64 {
    quantile_of(&sorted_copy(x), q)
}

/// sum_of_reoccurring_data_points:
/// np.unique lalu sum(unique[counts>1] * counts[counts>1])
pub fn sum_of_reoccurring_data_points(x: &[f64]) -> f64 {
    let mut counts: HashMap<u64, (f64, u32)> = HashMap::new();
    for &v in x {
        let key = v.to_bits();
        let e = counts.entry(key).or_insert((v, 0));
        e.1 += 1;
    }
    counts.values()
          .filter(|(_, c)| *c > 1)
          .map(|&(val, cnt)| val * cnt as f64)
          .sum()
}

/// (jumlah nilai unik yang muncul > 1 kali) / len(x)
pub fn percentage_of_reoccurring_values_to_all_values(x: &[f64]) -> f64 {
    if x.is_empty() { return f64::NAN; }
    let mut counts: HashMap<u64, u32> = HashMap::new();
    for &v in x { *counts.entry(v.to_bits()).or_insert(0) += 1; }
    counts.values().filter(|&&c| c > 1).count() as f64 / x.len() as f64
}

/// Energi segmen ke-`segment_focus` / total energi.
/// Pemisahan seperti np.array_split.
pub fn energy_ratio_by_chunks(x: &[f64], num_segments: usize, segment_focus: usize) -> f64 {
    let n = x.len();
    if n == 0 || num_segments == 0 { return f64::NAN; }
    let total: f64 = x.iter().map(|&v| v * v).sum();
    if total == 0.0 { return 0.0; }
    let base      = n / num_segments;
    let remainder = n % num_segments;
    let mut start = 0usize;
    for seg in 0..num_segments {
        let len = if seg < remainder { base + 1 } else { base };
        let end = (start + len).min(n);
        if seg == segment_focus {
            let seg_e: f64 = x[start..end].iter().map(|&v| v * v).sum();
            return seg_e / total;
        }
        start = end;
    }
    f64::NAN
}

/// Fitur change_quantiles — perubahan konsekutif di dalam koridor kuantil.
/// f_agg: "mean" | "var"  (np.mean / np.var ddof=0)
pub fn change_quantiles(x: &[f64], ql: f64, qh: f64, isabs: bool, f_agg: &str) -> f64 {
    if ql >= qh || x.is_empty() { return f64::NAN; }
    let s     = sorted_copy(x);
    let lower = quantile_of(&s, ql);
    let upper = quantile_of(&s, qh);
    let in_corridor: Vec<bool> = x.iter().map(|&v| v >= lower && v <= upper).collect();
    let diffs: Vec<f64> = (1..x.len())
        .filter(|&i| in_corridor[i - 1] && in_corridor[i])
        .map(|i| {
            let d = x[i] - x[i - 1];
            if isabs { d.abs() } else { d }
        })
        .collect();
    if diffs.is_empty() { return 0.0; }
    match f_agg {
        "mean" => mean(&diffs),
        "var"  => pop_var(&diffs),
        _      => f64::NAN,
    }
}

/// FFT pada sinyal nyata; attr: "real" | "imag".
pub fn fft_coefficient(x: &[f64], attr: &str, coeff: usize) -> f64 {
    let n = x.len();
    if coeff >= n { return f64::NAN; }
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    let mut buf: Vec<Complex<f64>> = x.iter().map(|&v| Complex::new(v, 0.0)).collect();
    fft.process(&mut buf);
    let c = buf[coeff];
    match attr { "real" => c.re, "imag" => c.im, _ => f64::NAN }
}

/// Index massa quantile: posisi kumulatif |x| mencapai q.
pub fn index_mass_quantile(x: &[f64], q: f64) -> f64 {
    let abs_sum: f64 = x.iter().map(|&v| v.abs()).sum();
    if abs_sum == 0.0 { return f64::NAN; }
    let mut cumsum = 0.0f64;
    for (i, &v) in x.iter().enumerate() {
        cumsum += v.abs();
        if cumsum / abs_sum >= q {
            return (i + 1) as f64 / x.len() as f64;
        }
    }
    1.0
}

/// Regresi linear pada seluruh deret; attr: "slope"|"intercept"|"rvalue"|"stderr".
pub fn linear_trend(x: &[f64], attr: &str) -> f64 {
    linreg_attr(x, attr)
}

/// agg_linear_trend: agregasi per chunk dengan f_agg_chunk, lalu regresi hasilnya.
/// f_agg_chunk: "max" | "min"
/// attr: "slope"|"intercept"|"rvalue"|"stderr"
pub fn agg_linear_trend(x: &[f64], attr: &str, chunk_len: usize, f_agg_chunk: &str) -> f64 {
    if chunk_len == 0 || x.is_empty() { return f64::NAN; }
    let n_chunks = x.len() / chunk_len;
    if n_chunks == 0 { return f64::NAN; }
    let aggregated: Vec<f64> = (0..n_chunks).map(|i| {
        let chunk = &x[i * chunk_len..(i + 1) * chunk_len];
        match f_agg_chunk {
            "max"  => chunk.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            "min"  => chunk.iter().cloned().fold(f64::INFINITY,     f64::min),
            "mean" => mean(chunk),
            _      => f64::NAN,
        }
    }).collect();
    linreg_attr(&aggregated, attr)
}

/// c3(lag) = mean(x[2l:] * x[l:-l] * x[:-2l])
pub fn c3(x: &[f64], lag: usize) -> f64 {
    let n = x.len();
    if n <= 2 * lag { return f64::NAN; }
    let vals: Vec<f64> = (0..n - 2 * lag)
        .map(|i| x[i + 2 * lag] * x[i + lag] * x[i])
        .collect();
    mean(&vals)
}

/// Welch PSD — sesuai scipy.signal.welch(x, nperseg=min(N,256)).
/// Mengembalikan nilai PSD (one-sided) pada indeks `coeff`.
pub fn spkt_welch_density(x: &[f64], coeff: usize) -> f64 {
    let n = x.len();
    if n < 2 { return f64::NAN; }
    let nperseg  = n.min(256);
    let noverlap = nperseg / 2;
    let step     = nperseg - noverlap;
    let nfft_out = nperseg / 2 + 1;
    if coeff >= nfft_out { return f64::NAN; }
    // Hann window
    let window: Vec<f64> = (0..nperseg)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / (nperseg - 1) as f64).cos()))
        .collect();
    let win_norm: f64 = window.iter().map(|&w| w * w).sum();
    let mut planner  = FftPlanner::new();
    let fft          = planner.plan_fft_forward(nperseg);
    let mut psd_sum  = vec![0.0f64; nfft_out];
    let mut seg_count = 0usize;
    let mut start = 0;
    while start + nperseg <= n {
        let mut buf: Vec<Complex<f64>> = x[start..start + nperseg]
            .iter().zip(window.iter())
            .map(|(&s, &w)| Complex::new(s * w, 0.0))
            .collect();
        fft.process(&mut buf);
        for i in 0..nfft_out { psd_sum[i] += buf[i].norm_sqr(); }
        seg_count += 1;
        start += step;
    }
    if seg_count == 0 { return f64::NAN; }
    let scale = 1.0 / (win_norm * seg_count as f64);
    let mut val = psd_sum[coeff] * scale;
    if coeff > 0 && coeff < nfft_out - 1 { val *= 2.0; } // one-sided
    val
}

/// AR(k) via OLS; mengembalikan koefisien indeks `coeff` (0-based, tanpa intercept).
/// Sesuai statsmodels AutoReg arparams.
pub fn ar_coefficient(x: &[f64], coeff: usize, k: usize) -> f64 {
    let n = x.len();
    if n <= k || coeff >= k { return f64::NAN; }
    let m = n - k; // observasi
    let p = k + 1; // intercept + k AR params
    let mut xtx = vec![vec![0.0f64; p]; p];
    let mut xty = vec![0.0f64; p];
    for t in 0..m {
        let yt = x[t + k];
        // row = [1, x[t+k-1], x[t+k-2], ..., x[t]]
        let row: Vec<f64> = std::iter::once(1.0)
            .chain((0..k).map(|lag| x[t + k - 1 - lag]))
            .collect();
        for i in 0..p {
            xty[i] += row[i] * yt;
            for j in 0..p { xtx[i][j] += row[i] * row[j]; }
        }
    }
    let beta = gauss_elim(&xtx, &xty);
    // beta[0]=intercept, beta[1..=k]=AR coefs; tsfresh indeks ke AR coef
    if beta.iter().any(|v| v.is_nan()) { return f64::NAN; }
    if coeff + 1 < beta.len() { beta[coeff + 1] } else { f64::NAN }
}

/// Ricker (Mexican hat) wavelet, identik dengan scipy.signal.ricker.
fn ricker(points: usize, width: f64) -> Vec<f64> {
    let a    = (2.0 / (3.0f64.sqrt() * width)) * PI.powf(0.25);
    let half = (points as f64 - 1.0) / 2.0;
    (0..points).map(|i| {
        let t   = i as f64 - half;
        let tsq = t * t;
        let wsq = width * width;
        a * (1.0 - tsq / wsq) * (-tsq / (2.0 * wsq)).exp()
    }).collect()
}

/// Konvolusi "same" (output panjang = len(signal)).
fn convolve_same(signal: &[f64], kernel: &[f64]) -> Vec<f64> {
    let sn = signal.len();
    let kn = kernel.len();
    if sn == 0 || kn == 0 { return vec![]; }
    let full_len = sn + kn - 1;
    let mut full = vec![0.0f64; full_len];
    for i in 0..sn {
        for j in 0..kn { full[i + j] += signal[i] * kernel[j]; }
    }
    let start = (kn - 1) / 2;
    full[start..start + sn].to_vec()
}

/// CWT dengan wavelet Ricker; mengembalikan koefisien [w_idx, coeff].
pub fn cwt_coefficients(x: &[f64], coeff: usize, w: usize, widths: &[usize]) -> f64 {
    let _w_idx = match widths.iter().position(|&wi| wi == w) {
        Some(i) => i,
        None    => return f64::NAN,
    };
    if coeff >= x.len() { return f64::NAN; }
    let width   = w as f64;
    let wav_len = ((10.0 * width) as usize).min(x.len()).max(1);
    // scipy: conj(wavelet[::-1]); Ricker riil dan simetris → reversed = asli
    let wav     = ricker(wav_len, width);
    let wav_rev: Vec<f64> = wav.iter().rev().cloned().collect();
    let row     = convolve_same(x, &wav_rev);
    if coeff < row.len() { row[coeff] } else { f64::NAN }
}

// ─── Fungsi utama: hitung semua 37 fitur ─────────────────────────────────────

/// Hitung 37 fitur dari rekaman [N×16].
/// Urutan output sesuai feature_meta.json["feature_names"].
pub fn compute_all(recording: &[Vec<f64>]) -> [f64; 37] {
    let n = recording.len();
    let mut ch: Vec<Vec<f64>> = vec![Vec::with_capacity(n); 16];
    for row in recording {
        for (c, &v) in row.iter().enumerate().take(16) { ch[c].push(v); }
    }
    [
        root_mean_square(&ch[10]),                                    // 0  MOS10__root_mean_square
        sum_of_reoccurring_data_points(&ch[6]),                       // 1  MOS6__sum_of_reoccurring_data_points
        change_quantiles(&ch[8],  0.2, 0.4, false, "var"),           // 2  MOS8__change_quantiles…var…False…qh0.4_ql0.2
        cwt_coefficients(&ch[15], 4, 5, &[2, 5, 10, 20]),            // 3  MOS15__cwt_coefficients…coeff_4__w_5
        agg_linear_trend(&ch[6],  "stderr", 5,  "max"),              // 4  MOS6__agg_linear_trend…stderr…chunk5…max
        change_quantiles(&ch[3],  0.4, 0.8, false, "var"),           // 5  MOS3__change_quantiles…var…False…qh0.8_ql0.4
        linear_trend(&ch[3],  "stderr"),                             // 6  MOS3__linear_trend…stderr
        ar_coefficient(&ch[13], 5, 10),                              // 7  MOS13__ar_coefficient…coeff5__k10
        sum_of_reoccurring_data_points(&ch[7]),                       // 8  MOS7__sum_of_reoccurring_data_points
        percentage_of_reoccurring_values_to_all_values(&ch[7]),       // 9  MOS7__percentage_of_reoccurring_values_to_all_values
        energy_ratio_by_chunks(&ch[2],  10, 4),                      // 10 MOS2__energy_ratio…seg4
        quantile(&ch[14], 0.9),                                       // 11 MOS14__quantile_0.9
        fft_coefficient(&ch[0],  "real", 1),                         // 12 MOS0__fft_coefficient…real…coeff1
        energy_ratio_by_chunks(&ch[3],  10, 1),                      // 13 MOS3__energy_ratio…seg1
        change_quantiles(&ch[0],  0.2, 0.6, true,  "mean"),          // 14 MOS0__change_quantiles…mean…True…qh0.6_ql0.2
        agg_linear_trend(&ch[0],  "rvalue", 50, "max"),              // 15 MOS0__agg_linear_trend…rvalue…chunk50…max
        index_mass_quantile(&ch[3],  0.2),                           // 16 MOS3__index_mass_quantile_0.2
        absolute_maximum(&ch[15]),                                    // 17 MOS15__absolute_maximum
        spkt_welch_density(&ch[3],  2),                              // 18 MOS3__spkt_welch_density…coeff2
        energy_ratio_by_chunks(&ch[6],  10, 6),                      // 19 MOS6__energy_ratio…seg6
        change_quantiles(&ch[8],  0.2, 0.6, true,  "var"),           // 20 MOS8__change_quantiles…var…True…qh0.6_ql0.2
        change_quantiles(&ch[6],  0.2, 0.6, false, "var"),           // 21 MOS6__change_quantiles…var…False…qh0.6_ql0.2
        agg_linear_trend(&ch[13], "stderr", 10, "min"),              // 22 MOS13__agg_linear_trend…stderr…chunk10…min
        absolute_maximum(&ch[8]),                                     // 23 MOS8__absolute_maximum
        maximum(&ch[3]),                                              // 24 MOS3__maximum
        change_quantiles(&ch[13], 0.0, 0.4, true,  "mean"),          // 25 MOS13__change_quantiles…mean…True…qh0.4_ql0.0
        quantile(&ch[3],  0.3),                                       // 26 MOS3__quantile_0.3
        fft_coefficient(&ch[0],  "imag", 1),                         // 27 MOS0__fft_coefficient…imag…coeff1
        agg_linear_trend(&ch[5],  "stderr", 50, "min"),              // 28 MOS5__agg_linear_trend…stderr…chunk50…min
        c3(&ch[13], 3),                                               // 29 MOS13__c3__lag_3
        quantile(&ch[11], 0.3),                                       // 30 MOS11__quantile_0.3
        minimum(&ch[6]),                                              // 31 MOS6__minimum
        change_quantiles(&ch[3],  0.4, 0.6, false, "var"),           // 32 MOS3__change_quantiles…var…False…qh0.6_ql0.4
        quantile(&ch[8],  0.8),                                       // 33 MOS8__quantile_0.8
        change_quantiles(&ch[3],  0.0, 0.4, false, "var"),           // 34 MOS3__change_quantiles…var…False…qh0.4_ql0.0
        energy_ratio_by_chunks(&ch[8],  10, 1),                      // 35 MOS8__energy_ratio…seg1
        energy_ratio_by_chunks(&ch[0],  10, 0),                      // 36 MOS0__energy_ratio…seg0
    ]
}
