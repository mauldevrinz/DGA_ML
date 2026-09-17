use pyo3::prelude::*;
use std::time::Instant;

// ── helpers (no_std-friendly math) ──────────────────────────────────────

fn f32_min(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}
fn f32_max(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}
fn f32_clamp(v: f32, lo: f32, hi: f32) -> f32 {
    f32_min(f32_max(v, lo), hi)
}

/// Triangular membership function: ramps up from `a` to `b`, back down to `c`.
fn trimf(x: f32, a: f32, b: f32, c: f32) -> f32 {
    if x <= a || x >= c {
        0.0
    } else if x <= b {
        (x - a) / (b - a)
    } else {
        (c - x) / (c - b)
    }
}

// ── Fuzzy-PID rule tables (MacMillan-style, 5×5) ───────────────────────
//
// Rows = error membership {NB, NS, ZE, PS, PB}
// Cols = delta-error membership {NB, NS, ZE, PS, PB}
// Values are consequent singletons in [-1, 1].

/// Kp adjustment: large |error| → increase Kp, small |error| → decrease.
const RULES_KP: [[f32; 5]; 5] = [
    [1.0, 1.0, 1.0, 0.5, 0.0],
    [1.0, 0.5, 0.5, 0.0, -0.5],
    [0.5, 0.0, 0.0, 0.0, -0.5],
    [-0.5, 0.0, 0.5, 0.5, 1.0],
    [0.0, 0.5, 1.0, 1.0, 1.0],
];

/// Ki adjustment: reduce Ki when error is large (anti-windup), increase near zero.
const RULES_KI: [[f32; 5]; 5] = [
    [-1.0, -0.5, 0.0, -0.5, -1.0],
    [-0.5, 0.0, 0.5, 0.0, -0.5],
    [0.0, 0.5, 1.0, 0.5, 0.0],
    [-0.5, 0.0, 0.5, 0.0, -0.5],
    [-1.0, -0.5, 0.0, -0.5, -1.0],
];

/// Kd adjustment: increase Kd when error changes fast, decrease near steady-state.
const RULES_KD: [[f32; 5]; 5] = [
    [1.0, 0.5, 0.0, 0.5, 1.0],
    [0.5, 0.0, -0.5, 0.0, 0.5],
    [0.0, -0.5, -1.0, -0.5, 0.0],
    [0.5, 0.0, -0.5, 0.0, 0.5],
    [1.0, 0.5, 0.0, 0.5, 1.0],
];

// ── PyO3 class ─────────────────────────────────────────────────────────

/// Fuzzy self-tuning PID controller.
///
/// The fuzzy inference engine adjusts the PID gains (Kp, Ki, Kd) online
/// based on the current error and its rate of change, using triangular
/// membership functions and weighted-average defuzzification.
///
/// Drop-in replacement for the old Python `FuzzyPIDController`.
#[pyclass]
pub struct FuzzyPIDController {
    kp0: f32,
    ki0: f32,
    kd0: f32,
    integral: f32,
    last_error: f32,
    last_time: Instant,
}

#[pymethods]
impl FuzzyPIDController {
    #[new]
    #[pyo3(signature = (kp=5.0, ki=0.5, kd=1.0))]
    fn new(kp: f32, ki: f32, kd: f32) -> Self {
        Self {
            kp0: kp,
            ki0: ki,
            kd0: kd,
            integral: 0.0,
            last_error: 0.0,
            last_time: Instant::now(),
        }
    }

    /// Compute PID output given a setpoint and current measured value.
    ///
    /// Returns an `i32` in the range -100..=100 representing the Peltier
    /// power percentage (positive = cool, negative = heat).
    fn compute(&mut self, setpoint: f32, current_value: f32) -> i32 {
        // ── time delta ──────────────────────────────────────────────
        let now = Instant::now();
        let dt = now.duration_since(self.last_time).as_secs_f32();
        let dt = if dt <= 0.0 { 0.01 } else { dt };
        self.last_time = now;

        // ── error / delta-error ─────────────────────────────────────
        let e = current_value - setpoint;
        let de = (e - self.last_error) / dt;

        // Normalise into [-1, 1] for fuzzy inference
        let e_n = f32_clamp(e / 10.0, -1.0, 1.0);
        let de_n = f32_clamp(de / 5.0, -1.0, 1.0);

        // ── fuzzification (5 triangular MFs) ────────────────────────
        let mfs = |x: f32| -> [f32; 5] {
            [
                trimf(x, -2.0, -1.0, -0.5), // NB
                trimf(x, -1.0, -0.5, 0.0),  // NS
                trimf(x, -0.5, 0.0, 0.5),   // ZE
                trimf(x, 0.0, 0.5, 1.0),    // PS
                trimf(x, 0.5, 1.0, 2.0),    // PB
            ]
        };

        let e_mfs = mfs(e_n);
        let de_mfs = mfs(de_n);

        // ── inference + defuzzification (weighted average) ──────────
        let mut num_kp = 0.0_f32;
        let mut num_ki = 0.0_f32;
        let mut num_kd = 0.0_f32;
        let mut den = 0.0_f32;

        for i in 0..5 {
            for j in 0..5 {
                let w = f32_min(e_mfs[i], de_mfs[j]); // AND = min
                den += w;
                num_kp += w * RULES_KP[i][j];
                num_ki += w * RULES_KI[i][j];
                num_kd += w * RULES_KD[i][j];
            }
        }

        let (dkp, dki, dkd) = if den > 0.0 {
            (num_kp / den, num_ki / den, num_kd / den)
        } else {
            (0.0, 0.0, 0.0)
        };

        // ── apply gain adjustments ──────────────────────────────────
        let kp = f32_max(self.kp0 + dkp * 5.0, 0.0);
        let ki = f32_max(self.ki0 + dki * 0.5, 0.0);
        let kd = f32_max(self.kd0 + dkd * 1.0, 0.0);

        // ── PID computation ─────────────────────────────────────────
        self.integral += e * dt;
        self.integral = f32_clamp(self.integral, -50.0, 50.0); // anti-windup

        let output = kp * e + ki * self.integral + kd * de;
        self.last_error = e;

        f32_clamp(output * 10.0, -100.0, 100.0) as i32
    }
}

/// Python module definition — `from fuzzy_pid import FuzzyPIDController`
#[pymodule]
fn fuzzy_pid(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<FuzzyPIDController>()?;
    Ok(())
}
