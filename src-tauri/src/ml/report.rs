//! PDF Report Generator — DGA Transformer Fault Diagnosis System
//! Generates a structured 3-page PDF report from model training results.

use printpdf::*;
use printpdf::path::{PaintMode, WindingOrder};
use std::fs::File;
use std::io::BufWriter;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ml::evaluation::EvaluationResults;
use crate::power_monitor::PowerSummary;

// ── A4 page geometry (mm) ─────────────────────────────────────────────────────
const PW: f32 = 210.0;
const PH: f32 = 297.0;
const ML: f32 = 15.0;
const MR: f32 = 15.0;
const CW: f32 = PW - ML - MR; // 180 mm

// ── Colour helpers ────────────────────────────────────────────────────────────
fn c_green() -> Color       { Color::Rgb(Rgb::new(0.133, 0.545, 0.133, None)) }
fn c_dark_green() -> Color  { Color::Rgb(Rgb::new(0.07,  0.37,  0.07,  None)) }
fn c_white() -> Color       { Color::Rgb(Rgb::new(1.0,   1.0,   1.0,   None)) }
fn c_black() -> Color       { Color::Rgb(Rgb::new(0.0,   0.0,   0.0,   None)) }
fn c_dark_gray() -> Color   { Color::Rgb(Rgb::new(0.35,  0.35,  0.35,  None)) }
fn c_mid_gray() -> Color    { Color::Rgb(Rgb::new(0.65,  0.65,  0.65,  None)) }
fn c_light_green() -> Color { Color::Rgb(Rgb::new(0.88,  0.96,  0.88,  None)) }
fn c_blue() -> Color        { Color::Rgb(Rgb::new(0.0,   0.30,  0.70,  None)) }
fn c_orange() -> Color      { Color::Rgb(Rgb::new(0.55,  0.22,  0.0,   None)) }
fn c_red() -> Color         { Color::Rgb(Rgb::new(0.75,  0.0,   0.0,   None)) }
fn c_light_red() -> Color   { Color::Rgb(Rgb::new(0.98,  0.88,  0.88,  None)) }

// ── Report data types ─────────────────────────────────────────────────────────

/// All model types supported for PDF generation.
pub enum TrainingReport {
    RandomForest {
        train_eval:         EvaluationResults,
        val_eval:           EvaluationResults,
        train_accuracy:     f32,
        val_accuracy:       f32,
        n_trees:            usize,
        training_secs:      f64,
        accuracy_curve:     Vec<(usize, f32, f32)>,
        power:              Option<PowerSummary>,
    },
    MLP {
        train_eval:     EvaluationResults,
        val_eval:       EvaluationResults,
        train_accuracy: f32,
        val_accuracy:   f32,
        train_loss:     f32,
        val_loss:       f32,
        n_epochs:       usize,
        training_secs:  f64,
        accuracy_curve: Vec<(usize, f32, f32)>,
        loss_curve:     Vec<(usize, f32, f32)>,
        power:          Option<PowerSummary>,
    },
    SVM {
        train_eval:         EvaluationResults,
        val_eval:           EvaluationResults,
        train_accuracy:     f32,
        val_accuracy:       f32,
        n_epochs:           usize,
        n_support_vectors:  usize,
        training_secs:      f64,
        final_loss:         f32,
        accuracy_curve:     Vec<(usize, f32, f32)>,
        power:              Option<PowerSummary>,
    },
    LSTM {
        train_eval:     EvaluationResults,
        val_eval:       EvaluationResults,
        train_accuracy: f32,
        val_accuracy:   f32,
        train_loss:     f32,
        val_loss:       f32,
        n_epochs:       usize,
        training_secs:  f64,
        accuracy_curve: Vec<(usize, f32, f32)>,
        loss_curve:     Vec<(usize, f32, f32)>,
        power:          Option<PowerSummary>,
    },
    CNN {
        train_eval:     EvaluationResults,
        val_eval:       EvaluationResults,
        train_accuracy: f32,
        val_accuracy:   f32,
        train_loss:     f32,
        val_loss:       f32,
        n_epochs:       usize,
        training_secs:  f64,
        accuracy_curve: Vec<(usize, f32, f32)>,
        loss_curve:     Vec<(usize, f32, f32)>,
        power:          Option<PowerSummary>,
    },
}

impl TrainingReport {
    fn model_name(&self) -> &str {
        match self {
            TrainingReport::RandomForest { .. } => "Random Forest",
            TrainingReport::MLP { .. }          => "Multi-Layer Perceptron (MLP)",
            TrainingReport::SVM { .. }          => "Support Vector Machine (SVM)",
            TrainingReport::LSTM { .. }         => "Long Short-Term Memory (LSTM)",
            TrainingReport::CNN { .. }          => "Convolutional Neural Network (1D-CNN)",
        }
    }
    fn model_short(&self) -> &str {
        match self {
            TrainingReport::RandomForest { .. } => "rf",
            TrainingReport::MLP { .. }          => "mlp",
            TrainingReport::SVM { .. }          => "svm",
            TrainingReport::LSTM { .. }         => "lstm",
            TrainingReport::CNN { .. }          => "cnn",
        }
    }
    fn train_accuracy(&self) -> f32 {
        match self {
            TrainingReport::RandomForest { train_accuracy, .. } |
            TrainingReport::MLP          { train_accuracy, .. } |
            TrainingReport::SVM          { train_accuracy, .. } |
            TrainingReport::LSTM         { train_accuracy, .. } |
            TrainingReport::CNN          { train_accuracy, .. } => *train_accuracy,
        }
    }
    fn val_accuracy(&self) -> f32 {
        match self {
            TrainingReport::RandomForest { val_accuracy, .. } |
            TrainingReport::MLP          { val_accuracy, .. } |
            TrainingReport::SVM          { val_accuracy, .. } |
            TrainingReport::LSTM         { val_accuracy, .. } |
            TrainingReport::CNN          { val_accuracy, .. } => *val_accuracy,
        }
    }
    fn training_secs(&self) -> f32 {
        match self {
            TrainingReport::RandomForest { training_secs, .. } |
            TrainingReport::MLP          { training_secs, .. } |
            TrainingReport::SVM          { training_secs, .. } |
            TrainingReport::LSTM         { training_secs, .. } |
            TrainingReport::CNN          { training_secs, .. } => *training_secs as f32,
        }
    }
    fn train_eval(&self) -> &EvaluationResults {
        match self {
            TrainingReport::RandomForest { train_eval, .. } |
            TrainingReport::MLP          { train_eval, .. } |
            TrainingReport::SVM          { train_eval, .. } |
            TrainingReport::LSTM         { train_eval, .. } |
            TrainingReport::CNN          { train_eval, .. } => train_eval,
        }
    }
    fn val_eval(&self) -> &EvaluationResults {
        match self {
            TrainingReport::RandomForest { val_eval, .. } |
            TrainingReport::MLP          { val_eval, .. } |
            TrainingReport::SVM          { val_eval, .. } |
            TrainingReport::LSTM         { val_eval, .. } |
            TrainingReport::CNN          { val_eval, .. } => val_eval,
        }
    }
    fn power(&self) -> Option<&PowerSummary> {
        match self {
            TrainingReport::RandomForest { power, .. } |
            TrainingReport::MLP          { power, .. } |
            TrainingReport::SVM          { power, .. } |
            TrainingReport::LSTM         { power, .. } |
            TrainingReport::CNN          { power, .. } => power.as_ref(),
        }
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

/// Approximate rendered text width in mm (Helvetica, ~55% char width ratio).
fn aw(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * 0.55 * 0.3528
}

/// Format waktu training adaptif, SAMA dengan footer GUI:
///   < 1 detik     → milidetik (mis. "340 ms")
///   < 60 detik    → detik     (mis. "12.34 s")
///   >= 60 detik   → menit     (mis. "3.05 min")
fn format_training_time(secs: f32) -> String {
    if secs < 1.0 {
        format!("{:.0} ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.2} s", secs)
    } else {
        format!("{:.2} min", secs / 60.0)
    }
}

/// Unix timestamp to "YYYY-MM-DD HH:MM:SS"
fn unix_to_date(ts: u64) -> String {
    let h = (ts % 86400) / 3600;
    let m = (ts % 3600) / 60;
    let s = ts % 60;
    let mut days = ts / 86400;
    let mut year = 1970u32;
    loop {
        let dy: u64 = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let dim: [u64; 12] = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    let mut day = days + 1;
    for &d in &dim {
        if day <= d { break; }
        day -= d;
        month += 1;
    }
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, h, m, s)
}

/// Select up to `n` evenly-spaced indices (always includes first and last).
#[allow(dead_code)]
fn sample_indices(len: usize, n: usize) -> Vec<usize> {
    if len == 0 { return vec![]; }
    if len <= n  { return (0..len).collect(); }
    let mut v = vec![0usize];
    for i in 1..n - 1 { v.push(i * (len - 1) / (n - 1)); }
    v.push(len - 1);
    v.dedup();
    v
}

// ── Low-level drawing primitives ──────────────────────────────────────────────

fn rect_points(x: f32, y: f32, w: f32, h: f32) -> Vec<(Point, bool)> {
    vec![
        (Point::new(Mm(x),     Mm(y)),     false),
        (Point::new(Mm(x + w), Mm(y)),     false),
        (Point::new(Mm(x + w), Mm(y - h)), false),
        (Point::new(Mm(x),     Mm(y - h)), false),
    ]
}

/// Draw a filled rectangle.
fn fill_rect(l: &PdfLayerReference, x: f32, y: f32, w: f32, h: f32, color: Color) {
    l.set_fill_color(color);
    l.add_polygon(Polygon {
        rings: vec![rect_points(x, y, w, h)],
        mode: PaintMode::Fill,
        winding_order: WindingOrder::NonZero,
    });
}

/// Draw a stroked rectangle (border only).
fn stroke_rect(l: &PdfLayerReference, x: f32, y: f32, w: f32, h: f32,
               color: Color, thick: f32) {
    l.set_outline_color(color);
    l.set_outline_thickness(thick);
    l.add_polygon(Polygon {
        rings: vec![rect_points(x, y, w, h)],
        mode: PaintMode::Stroke,
        winding_order: WindingOrder::NonZero,
    });
}

/// Draw a horizontal line.
fn hline(l: &PdfLayerReference, x1: f32, x2: f32, y: f32, color: Color, thick: f32) {
    l.set_outline_color(color);
    l.set_outline_thickness(thick);
    l.add_line(Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y)), false),
            (Point::new(Mm(x2), Mm(y)), false),
        ],
        is_closed: false,
    });
}

/// Draw a vertical line.
fn vline(l: &PdfLayerReference, x: f32, y1: f32, y2: f32, color: Color, thick: f32) {
    l.set_outline_color(color);
    l.set_outline_thickness(thick);
    l.add_line(Line {
        points: vec![
            (Point::new(Mm(x), Mm(y1)), false),
            (Point::new(Mm(x), Mm(y2)), false),
        ],
        is_closed: false,
    });
}

/// Draw text at absolute position with explicit colour.
fn txt(l: &PdfLayerReference, s: &str, size: f32, x: f32, y: f32,
       font: &IndirectFontRef, color: Color) {
    l.set_fill_color(color);
    l.use_text(s, size, Mm(x), Mm(y), font);
}

/// Draw text horizontally centred on the page.
fn txt_center(l: &PdfLayerReference, s: &str, size: f32, y: f32,
              font: &IndirectFontRef, color: Color) {
    let x = ((PW - aw(s, size)) / 2.0).max(ML);
    txt(l, s, size, x, y, font, color);
}

/// Draw text right-aligned to `rx`.
fn txt_right(l: &PdfLayerReference, s: &str, size: f32, rx: f32, y: f32,
             font: &IndirectFontRef, color: Color) {
    let x = (rx - aw(s, size)).max(ML);
    txt(l, s, size, x, y, font, color);
}

/// Draw text rotated 90° counter-clockwise (untuk label sumbu-Y).
/// `(x, y)` adalah titik basis teks (kiri-bawah) dalam mm.
fn draw_vertical_text(l: &PdfLayerReference, s: &str, size: f32,
                      x: f32, y: f32, font: &IndirectFontRef, color: Color) {
    l.set_fill_color(color);
    l.begin_text_section();
    l.set_font(font, size);
    l.set_text_matrix(TextMatrix::TranslateRotate(
        Mm(x).into_pt(), Mm(y).into_pt(), 90.0));
    l.write_text(s, font);
    l.end_text_section();
}

// ── Composite components ──────────────────────────────────────────────────────

fn draw_footer(l: &PdfLayerReference, f: &IndirectFontRef, page: u32, total: u32) {
    hline(l, ML, PW - MR, 14.0, c_mid_gray(), 0.3);
    txt(l, "DGA Transformer Fault Diagnosis System", 7.0, ML, 10.0, f, c_dark_gray());
    txt_right(l, &format!("Page {} / {}", page, total), 7.0, PW - MR, 10.0, f, c_dark_gray());
}

fn page_subheader(l: &PdfLayerReference, fb: &IndirectFontRef, title: &str) {
    fill_rect(l, 0.0, PH, PW, 18.0, c_dark_green());
    txt_center(l, title, 11.0, PH - 12.0, fb, c_white());
}

/// Section title with green underline. Returns Y below.
fn section_title(l: &PdfLayerReference, fb: &IndirectFontRef, title: &str, y: f32) -> f32 {
    txt(l, title, 10.5, ML, y, fb, c_dark_green());
    let y2 = y - 4.0;
    hline(l, ML, PW - MR, y2, c_green(), 0.6);
    y2 - 3.0
}

/// Coloured metric summary box.
fn metric_box(l: &PdfLayerReference,
              f: &IndirectFontRef, fb: &IndirectFontRef,
              x: f32, y: f32, w: f32, h: f32,
              label: &str, value: &str, color: Color) {
    fill_rect(l, x, y, w, h, color);
    let lx = (x + (w - aw(label, 7.5)) / 2.0).max(x + 1.0);
    txt(l, label, 7.5, lx, y - 7.0, f, c_white());
    let vx = (x + (w - aw(value, 15.0)) / 2.0).max(x + 1.0);
    txt(l, value, 15.0, vx, y - h + 6.0, fb, c_white());
}

fn eval_table(l: &PdfLayerReference,
              f: &IndirectFontRef, fb: &IndirectFontRef,
              eval: &EvaluationResults, label: &str, y: f32) -> f32 {
    let heading = format!("{} - Accuracy: {:.2}%", label, eval.accuracy * 100.0);
    let y0 = section_title(l, fb, &heading, y);
    let cw = [36.0f32, 36.0, 36.0, 36.0, 36.0];
    let tw: f32 = cw.iter().sum();
    let rh = 7.5;

    fill_rect(l, ML, y0, tw, rh, c_dark_green());
    let hdrs = ["Metric", "Baseline", "Normal", "Overheating", "Arcing"];
    let mut hx = ML;
    for (&w, h) in cw.iter().zip(hdrs.iter()) {
        txt(l, h, 8.5, hx + 2.0, y0 - rh + 2.5, fb, c_white());
        hx += w;
    }

    let rows: [(&str, String, String, String, String); 4] = [
        ("Precision",
         format!("{:.4}", eval.class_metrics[0].precision),
         format!("{:.4}", eval.class_metrics[1].precision),
         format!("{:.4}", eval.class_metrics[2].precision),
         format!("{:.4}", eval.class_metrics[3].precision)),
        ("Recall",
         format!("{:.4}", eval.class_metrics[0].recall),
         format!("{:.4}", eval.class_metrics[1].recall),
         format!("{:.4}", eval.class_metrics[2].recall),
         format!("{:.4}", eval.class_metrics[3].recall)),
        ("F1-Score",
         format!("{:.4}", eval.class_metrics[0].f1_score),
         format!("{:.4}", eval.class_metrics[1].f1_score),
         format!("{:.4}", eval.class_metrics[2].f1_score),
         format!("{:.4}", eval.class_metrics[3].f1_score)),
        ("Support",
         format!("{}", eval.class_metrics[0].support),
         format!("{}", eval.class_metrics[1].support),
         format!("{}", eval.class_metrics[2].support),
         format!("{}", eval.class_metrics[3].support)),
    ];

    let mut ry = y0 - rh;
    for (i, row) in rows.iter().enumerate() {
        let bg = if i % 2 == 0 { c_light_green() } else { c_white() };
        fill_rect(l, ML, ry, tw, rh, bg);
        let mut rx = ML;
        txt(l, row.0, 8.5, rx + 2.0, ry - rh + 2.5, fb, c_dark_green()); rx += cw[0];
        txt(l, &row.1, 8.5, rx + 2.0, ry - rh + 2.5, f,  c_black());      rx += cw[1];
        txt(l, &row.2, 8.5, rx + 2.0, ry - rh + 2.5, f,  c_black());      rx += cw[2];
        txt(l, &row.3, 8.5, rx + 2.0, ry - rh + 2.5, f,  c_black());      rx += cw[3];
        txt(l, &row.4, 8.5, rx + 2.0, ry - rh + 2.5, f,  c_black());
        let _ = rx;
        ry -= rh;
    }

    stroke_rect(l, ML, y0, tw, rh * 5.0, c_dark_gray(), 0.3);
    let mut vx = ML;
    for &w in &cw[..cw.len() - 1] {
        vx += w;
        vline(l, vx, y0, y0 - rh * 5.0, c_dark_gray(), 0.3);
    }
    for i in 1..5 { hline(l, ML, ML + tw, y0 - rh * i as f32, c_dark_gray(), 0.3); }

    ry - 4.0
}

/// 4x4 confusion matrix. Returns Y below.
fn confusion_matrix_grid(l: &PdfLayerReference,
                         f: &IndirectFontRef, fb: &IndirectFontRef,
                         matrix: [[usize; 4]; 4], caption: Option<&str>, y: f32) -> f32 {
    let y0 = match caption {
        Some(c) => section_title(l, fb, c, y),
        None     => y,
    };
    let cw = 32.0f32; // cell width
    let ch = 10.0f32; // cell height
    let ox = ML + 5.0; // offset x

    let class_names = ["Baseline", "Normal", "Overheating", "Arcing"];

    // Draw Headers (Predicted)
    for (i, name) in class_names.iter().enumerate() {
        fill_rect(l, ox + cw * (i as f32 + 1.0), y0, cw, ch, c_dark_green());
        txt(l, name, 7.5, ox + cw * (i as f32 + 1.0) + 2.0, y0 - ch + 3.5, fb, c_white());
    }

    // Draw Rows (Actual)
    for i in 0..4 {
        let ry = y0 - ch * (i as f32 + 1.0);
        // Row Header
        fill_rect(l, ox, ry, cw, ch, c_dark_green());
        txt(l, class_names[i], 7.5, ox + 2.0, ry - ch + 3.5, fb, c_white());
        
        // Cells
        for j in 0..4 {
            let cx = ox + cw * (j as f32 + 1.0);
            let bg = if i == j { c_light_green() } else if matrix[i][j] > 0 { c_light_red() } else { c_white() };
            fill_rect(l, cx, ry, cw, ch, bg);
            let val = format!("{}", matrix[i][j]);
            let fg = if i == j { c_dark_green() } else if matrix[i][j] > 0 { c_red() } else { c_black() };
            txt(l, &val, 11.0, cx + (cw - aw(&val, 11.0)) / 2.0, ry - ch + 3.0, fb, fg);
        }
    }

    // Draw Grid Lines
    stroke_rect(l, ox, y0, cw * 5.0, ch * 5.0, c_dark_gray(), 0.4);
    for i in 1..5 {
        vline(l, ox + cw * i as f32, y0, y0 - ch * 5.0, c_dark_gray(), 0.3);
        hline(l, ox, ox + cw * 5.0, y0 - ch * i as f32, c_dark_gray(), 0.3);
    }

    let ly = y0 - ch * 5.0 - 5.0;
    txt(l, "Rows: Actual Class, Columns: Predicted Class", 7.5, ML, ly, f, c_dark_gray());
    txt(l, "Green: True Positives | Red: Misclassifications", 7.5, ML + 90.0, ly, f, c_dark_gray());

    ly - 9.0
}

/// Generic ranked table with optional bar-chart column (bar_col = column index + max value).
fn ranked_table(l: &PdfLayerReference,
                f: &IndirectFontRef, fb: &IndirectFontRef,
                headers: &[&str], widths: &[f32],
                rows: &[Vec<String>],
                bar_col: Option<(usize, f32)>,
                y: f32) -> f32 {
    let tw: f32 = widths.iter().sum();
    let rh = 7.0;

    fill_rect(l, ML, y, tw, rh, c_dark_green());
    let mut hx = ML;
    for (&w, h) in widths.iter().zip(headers.iter()) {
        txt(l, h, 8.5, hx + 2.0, y - rh + 2.5, fb, c_white());
        hx += w;
    }

    let max_bar = bar_col.map(|(_, mv)| mv).unwrap_or(1.0f32);
    let bar_idx = bar_col.map(|(i, _)| i);
    let mut ry = y - rh;

    for (i, row) in rows.iter().enumerate() {
        let bg = if i % 2 == 0 { c_light_green() } else { c_white() };
        fill_rect(l, ML, ry, tw, rh, bg);
        let mut rx = ML;
        for (ci, (&w, cell)) in widths.iter().zip(row.iter()).enumerate() {
            if bar_idx == Some(ci) {
                if let Ok(val) = cell.parse::<f32>() {
                    let bar_px = (w - 28.0).max(5.0);
                    let bw = if max_bar > 0.0 { bar_px * (val / max_bar) } else { 0.0 };
                    fill_rect(l, rx + 2.0, ry - 1.5, bw, rh - 3.0, c_green());
                    txt(l, &format!("{:.4}", val), 6.5, rx + bw + 4.0, ry - rh + 2.5, f, c_dark_gray());
                }
            } else {
                txt(l, cell, 8.5, rx + 2.0, ry - rh + 2.5, f, c_black());
            }
            rx += w;
        }
        let _ = rx;
        ry -= rh;
    }

    stroke_rect(l, ML, y, tw, rh * (rows.len() + 1) as f32, c_dark_gray(), 0.3);
    let mut vx = ML;
    for &w in &widths[..widths.len() - 1] {
        vx += w;
        vline(l, vx, y, y - rh * (rows.len() + 1) as f32, c_dark_gray(), 0.3);
    }
    for i in 1..=rows.len() {
        hline(l, ML, ML + tw, y - rh * i as f32, c_dark_gray(), 0.3);
    }
    ry - 4.0
}

/// Compact per-epoch table (baris rapat). Returns Y below.
fn compact_epoch_table(l: &PdfLayerReference,
                       f: &IndirectFontRef, fb: &IndirectFontRef,
                       headers: &[&str], widths: &[f32],
                       rows: &[Vec<String>], y: f32) -> f32 {
    let rh = 4.6;
    let tw: f32 = widths.iter().sum();
    // Header
    fill_rect(l, ML, y, tw, rh, c_dark_green());
    let mut hx = ML;
    for (i, h) in headers.iter().enumerate() {
        txt(l, h, 7.0, hx + 2.0, y - rh + 1.6, fb, c_white());
        hx += widths[i];
    }
    let mut ry = y - rh;
    for (ri, row) in rows.iter().enumerate() {
        let bg = if ri % 2 == 0 { c_light_green() } else { c_white() };
        fill_rect(l, ML, ry, tw, rh, bg);
        let mut rx = ML;
        for (ci, cell) in row.iter().enumerate() {
            txt(l, cell, 6.8, rx + 2.0, ry - rh + 1.5, f, c_black());
            rx += widths[ci];
        }
        ry -= rh;
    }
    // Border & garis kolom
    stroke_rect(l, ML, y, tw, rh * (rows.len() + 1) as f32, c_dark_gray(), 0.3);
    let mut vx = ML;
    for &w in &widths[..widths.len() - 1] {
        vx += w;
        vline(l, vx, y, y - rh * (rows.len() + 1) as f32, c_dark_gray(), 0.3);
    }
    ry - 4.0
}

/// 2-column key-value info table. Returns Y below.
fn kv_table(l: &PdfLayerReference,
            f: &IndirectFontRef, fb: &IndirectFontRef,
            rows: &[(&str, String)], y: f32) -> f32 {
    let rh = 7.5;
    let c0 = 72.0;
    let tw = CW;
    for (i, (k, v)) in rows.iter().enumerate() {
        let ry = y - i as f32 * rh;
        let bg = if i % 2 == 0 { c_light_green() } else { c_white() };
        fill_rect(l, ML, ry, tw, rh, bg);
        txt(l, k, 8.5, ML + 2.0, ry - rh + 2.5, fb, c_dark_green());
        txt(l, v, 8.5, ML + c0,  ry - rh + 2.5, f,  c_black());
    }
    stroke_rect(l, ML, y, tw, rh * rows.len() as f32, c_dark_gray(), 0.3);
    vline(l, ML + c0, y, y - rh * rows.len() as f32, c_dark_gray(), 0.3);
    for i in 1..rows.len() {
        hline(l, ML, ML + tw, y - rh * i as f32, c_dark_gray(), 0.3);
    }
    y - rh * rows.len() as f32 - 4.0
}

/// Accuracy checkpoint table (10% interval sampling). Returns Y below.
fn accuracy_checkpoint_table(l: &PdfLayerReference,
                             f: &IndirectFontRef, fb: &IndirectFontRef,
                             x_label: &str, curve: &[(usize, f32, f32)], y: f32) -> f32 {
    if curve.is_empty() { return y; }
    let y0 = section_title(l, fb, "Accuracy Curve (Checkpoint)", y);
    let widths = [28.0f32, 76.0, 76.0];
    let hdrs = [x_label, "Training Accuracy (%)", "Testing Accuracy (%)"];
    
    // Limit to max 100 epochs
    let _max_epoch = curve.iter().map(|(x, _, _)| *x).max().unwrap_or(1);
    let filtered_curve: Vec<_> = curve.iter()
        .filter(|(x, _, _)| *x <= 100)
        .collect();
    
    if filtered_curve.is_empty() { return y0 - 5.0; }
    
    // Take every 5 epochs (checkpoint sampling)
    let rows: Vec<Vec<String>> = filtered_curve.iter()
        .filter(|(epoch, _, _)| epoch % 5 == 1 || *epoch == 100)
        .map(|(x, ta, va)| {
            vec![format!("{}", x), format!("{:.2}", ta * 100.0), format!("{:.2}", va * 100.0)]
        })
        .collect();
    
    ranked_table(l, f, fb, &hdrs, &widths, &rows, None, y0)
}

/// Accuracy vs X curve table. Returns Y below.
#[allow(dead_code)]
fn accuracy_curve_table(l: &PdfLayerReference,
                        f: &IndirectFontRef, fb: &IndirectFontRef,
                        x_label: &str, curve: &[(usize, f32, f32)], y: f32) -> f32 {
    if curve.is_empty() { return y; }
    let y0 = section_title(l, fb, "Accuracy Curve (All Epochs)", y);
    let widths = [28.0f32, 76.0, 76.0];
    let hdrs = [x_label, "Training Accuracy (%)", "Testing Accuracy (%)"];
    // Limit to first 100 epochs
    let rows: Vec<Vec<String>> = curve.iter()
        .filter(|(x, _, _)| *x <= 100)
        .map(|(x, ta, va)| {
            vec![format!("{}", x), format!("{:.2}", ta * 100.0), format!("{:.2}", va * 100.0)]
        }).collect();
    ranked_table(l, f, fb, &hdrs, &widths, &rows, None, y0)
}

/// Loss checkpoint curve table (10% interval sampling). Returns Y below.
fn loss_checkpoint_table(l: &PdfLayerReference,
                         f: &IndirectFontRef, fb: &IndirectFontRef,
                         curve: &[(usize, f32, f32)], y: f32) -> f32 {
    if curve.is_empty() { return y; }
    let y0 = section_title(l, fb, "Loss Curve (Checkpoint)", y);
    let widths = [28.0f32, 76.0, 76.0];
    let hdrs = ["Epoch", "Training Loss", "Testing Loss"];
    
    // Limit to max 100 epochs
    let filtered_curve: Vec<_> = curve.iter()
        .filter(|(ep, _, _)| *ep <= 100)
        .collect();
    
    if filtered_curve.is_empty() { return y0 - 5.0; }
    
    // Take every 5 epochs (checkpoint sampling)
    let rows: Vec<Vec<String>> = filtered_curve.iter()
        .filter(|(epoch, _, _)| epoch % 5 == 1 || *epoch == 100)
        .map(|(ep, tl, vl)| {
            vec![format!("{}", ep), format!("{:.6}", tl), format!("{:.6}", vl)]
        })
        .collect();
    
    ranked_table(l, f, fb, &hdrs, &widths, &rows, None, y0)
}

/// Loss curve table. Returns Y below.
#[allow(dead_code)]
fn loss_curve_table(l: &PdfLayerReference,
                    f: &IndirectFontRef, fb: &IndirectFontRef,
                    curve: &[(usize, f32, f32)], y: f32) -> f32 {
    if curve.is_empty() { return y; }
    let y0 = section_title(l, fb, "Loss Curve (All Epochs)", y);
    let widths = [28.0f32, 76.0, 76.0];
    let hdrs = ["Epoch", "Training Loss", "Testing Loss"];
    // Limit to first 100 epochs
    let rows: Vec<Vec<String>> = curve.iter()
        .filter(|(ep, _, _)| *ep <= 100)
        .map(|(ep, tl, vl)| {
            vec![format!("{}", ep), format!("{:.6}", tl), format!("{:.6}", vl)]
        }).collect();
    ranked_table(l, f, fb, &hdrs, &widths, &rows, None, y0)
}

// ── Page builders ─────────────────────────────────────────────────────────────

/// Power consumption section. Returns Y below.
fn draw_power_section(l: &PdfLayerReference,
                      f: &IndirectFontRef, fb: &IndirectFontRef,
                      ps: &PowerSummary, y: f32) -> f32 {
    let y0 = section_title(l, fb, "POWER CONSUMPTION DURING TRAINING", y);
    let y0 = y0 - 2.0;

    // 4-row x 4-column layout: [label | value | label | value]
    let cw = [62.0f32, 28.0, 62.0, 28.0];
    let tw: f32 = cw.iter().sum(); // = 180 mm = CW
    let rh = 7.5f32;

    let energy_wh = ps.energy_joules / 3600.0;
    let dur_str = if ps.duration_secs < 60.0 {
        format!("{:.1} s", ps.duration_secs)
    } else {
        format!("{:.1} min", ps.duration_secs / 60.0)
    };

    let rows: [(&str, String, &str, String); 4] = [
        ("Avg Total Power",    format!("{:.2} W",   ps.avg_total_w),
         "Avg CPU Temp",       format!("{:.1} C",  ps.avg_cpu_temp)),
        ("Peak Total Power",   format!("{:.2} W",   ps.peak_total_w),
         "Peak CPU Temp",      format!("{:.1} C",  ps.peak_cpu_temp)),
        ("Avg CPU+GPU Power",  format!("{:.2} W",   ps.avg_cpu_gpu_w),
         "Energy Used",        format!("{:.1} J  ({:.4} Wh)", ps.energy_joules, energy_wh)),
        ("Peak CPU+GPU Power", format!("{:.2} W",   ps.peak_cpu_gpu_w),
         "Training Duration",  dur_str),
    ];

    // Header bar
    fill_rect(l, ML, y0, tw, rh, c_orange());
    let hdrs = ["Power Metric", "Value", "Thermal / Energy", "Value"];
    let mut hx = ML;
    for (&w, h) in cw.iter().zip(hdrs.iter()) {
        txt(l, h, 8.5, hx + 2.0, y0 - rh + 2.5, fb, c_white());
        hx += w;
    }

    // Data rows
    let mut ry = y0 - rh;
    for (i, (l1, v1, l2, v2)) in rows.iter().enumerate() {
        let bg = if i % 2 == 0 { c_light_green() } else { c_white() };
        fill_rect(l, ML, ry, tw, rh, bg);
        let mut rx = ML;
        txt(l, l1, 8.5, rx + 2.0, ry - rh + 2.5, fb, c_dark_green()); rx += cw[0];
        txt(l, v1, 8.5, rx + 2.0, ry - rh + 2.5, f,  c_black());      rx += cw[1];
        txt(l, l2, 8.5, rx + 2.0, ry - rh + 2.5, fb, c_dark_green()); rx += cw[2];
        txt(l, v2, 8.5, rx + 2.0, ry - rh + 2.5, f,  c_black());
        let _ = rx;
        ry -= rh;
    }

    // Grid lines
    stroke_rect(l, ML, y0, tw, rh * 5.0, c_dark_gray(), 0.3);
    let mut vx = ML;
    for &w in &cw[..3] {
        vx += w;
        vline(l, vx, y0, y0 - rh * 5.0, c_dark_gray(), 0.3);
    }
    for i in 1..5 {
        hline(l, ML, ML + tw, y0 - rh * i as f32, c_dark_gray(), 0.3);
    }

    ry - 4.0
}


fn draw_page1(l: &PdfLayerReference,
              f: &IndirectFontRef, fb: &IndirectFontRef,
              report: &TrainingReport, ts: u64) {
    fill_rect(l, 0.0, PH, PW, 5.0, c_dark_green());
    fill_rect(l, 0.0, PH - 5.0, PW, 58.0, c_green());
    txt_center(l, "MODEL TRAINING REPORT", 20.0, PH - 20.0, fb, c_white());
    txt_center(l, "DGA Transformer Fault Diagnosis System", 10.0, PH - 31.0, f, c_white());
    txt_center(l, &format!("Model: {}", report.model_name()), 13.0, PH - 43.0, fb, c_white());
    txt_center(l, &format!("Date: {}", unix_to_date(ts)), 8.5, PH - 54.0, f, c_white());

    let mut y = PH - 72.0;
    y = section_title(l, fb, "TRAINING SUMMARY", y);
    y -= 3.0;

    let bw = (CW - 10.0) / 3.0;
    let bh = 26.0;
    let secs = report.training_secs();
    let time_str = format_training_time(secs);
    metric_box(l, f, fb, ML,                    y, bw, bh, "Training Accuracy",
               &format!("{:.1}%", report.train_accuracy() * 100.0), c_dark_green());
    metric_box(l, f, fb, ML + bw + 5.0,         y, bw, bh, "Testing Accuracy",
               &format!("{:.1}%", report.val_accuracy() * 100.0), c_blue());
    metric_box(l, f, fb, ML + (bw + 5.0) * 2.0, y, bw, bh, "Training Time",
               &time_str, c_orange());
    y = y - bh - 8.0;

    y = section_title(l, fb, "MODEL CONFIGURATION", y);
    y -= 2.0;

    let cfg: Vec<(&str, String)> = match report {
        TrainingReport::RandomForest { n_trees, .. } => vec![
            ("Model Type",          "Random Forest".to_string()),
            ("Number of Trees",     format!("{}", n_trees)),
            ("Data Split",          "80% Training / 20% Testing".to_string()),
            ("Extracted Features",  "mean, std, min, max, range, median".to_string()),
            ("Feature Dimensions",  "6 statistics x 8 sensors = 48 features".to_string()),
        ],
        TrainingReport::MLP { n_epochs, .. } => vec![
            ("Model Type",    "Multi-Layer Perceptron (MLP)".to_string()),
            ("Epochs",        format!("{}", n_epochs)),
            ("Data Split",    "80% Training / 20% Testing".to_string()),
            ("Architecture",  "Input -> Hidden Layers -> Sigmoid -> Output".to_string()),
            ("Input Shape",   "8 sensors x 300 timesteps (flattened -> 2400)".to_string()),
        ],
        TrainingReport::SVM { n_epochs, n_support_vectors, final_loss, .. } => vec![
            ("Model Type",          "Support Vector Machine (SVM)".to_string()),
            ("Epochs",              format!("{}", n_epochs)),
            ("Support Vectors",     format!("{}", n_support_vectors)),
            ("Final Training Loss", format!("{:.5}", final_loss)),
            ("Data Split",          "80% Training / 20% Testing".to_string()),
        ],
        TrainingReport::LSTM { n_epochs, .. } => vec![
            ("Model Type",      "Long Short-Term Memory (LSTM)".to_string()),
            ("Epochs",          format!("{}", n_epochs)),
            ("Data Split",      "80% Training / 20% Testing".to_string()),
            ("Input Shape",     "8 sensors x 300 timesteps".to_string()),
            ("Architecture",    "Recurrent Neural Network (LSTM)".to_string()),
        ],
        TrainingReport::CNN { n_epochs, .. } => vec![
            ("Model Type",      "1D Convolutional Neural Network (1D-CNN)".to_string()),
            ("Epochs",          format!("{}", n_epochs)),
            ("Data Split",      "80% Training / 20% Testing".to_string()),
            ("Input Shape",     "8 sensors x 300 timesteps".to_string()),
            ("Architecture",    "1D-CNN + Backpropagation (SGD, Cross Entropy)".to_string()),
        ],
    };

    let rh = 7.5;
    let c0 = 68.0;
    for (i, (k, v)) in cfg.iter().enumerate() {
        let ry = y - i as f32 * rh;
        let bg = if i % 2 == 0 { c_light_green() } else { c_white() };
        fill_rect(l, ML, ry, CW, rh, bg);
        txt(l, k, 8.5, ML + 2.0, ry - rh + 2.5, fb, c_dark_green());
        txt(l, v, 8.5, ML + c0,  ry - rh + 2.5, f,  c_black());
    }
    stroke_rect(l, ML, y, CW, rh * cfg.len() as f32, c_dark_gray(), 0.3);
    vline(l, ML + c0, y, y - rh * cfg.len() as f32, c_dark_gray(), 0.3);
    for i in 1..cfg.len() { hline(l, ML, ML + CW, y - rh * i as f32, c_dark_gray(), 0.3); }
    
    draw_footer(l, f, 1, 4);
}

fn draw_page2(l: &PdfLayerReference,
              f: &IndirectFontRef, fb: &IndirectFontRef,
              report: &TrainingReport) {
    page_subheader(l, fb, "GAS SENSORS & POWER CONSUMPTION");
    let mut y = PH - 25.0;
    
    y = section_title(l, fb, "GAS SENSORS USED (8 Sensors)", y);
    y -= 2.0;
    let sensors = ["TGS2600", "MQ135", "MQ3", "MQ6", "MQ7", "TGS2602", "TGS2611", "TGS2620"];
    for (i, s) in sensors.iter().enumerate() {
        let bx = ML + (i % 4) as f32 * 46.0;
        let by = y - 2.0 - (i / 4) as f32 * 10.0;
        fill_rect(l, bx, by, 43.0, 8.0, c_dark_green());
        txt(l, s, 8.5, bx + 3.0, by - 5.5, fb, c_white());
    }
    
    let y_after_sensors = y - 28.0;
    if let Some(ps) = report.power() {
        draw_power_section(l, f, fb, ps, y_after_sensors);
    }
    
    draw_footer(l, f, 2, 4);
}

fn draw_page3(l: &PdfLayerReference,
              f: &IndirectFontRef, fb: &IndirectFontRef,
              report: &TrainingReport) {
    page_subheader(l, fb, "MODEL EVALUATION RESULTS");
    let mut y = PH - 25.0;
    y = eval_table(l, f, fb, report.train_eval(), "Training Data", y);
    y -= 8.0;
    y = eval_table(l, f, fb, report.val_eval(), "Testing Data", y);
    y -= 8.0;
    confusion_matrix_grid(l, f, fb, report.val_eval().confusion_matrix,
                          Some("Confusion Matrix (Testing Data)"), y);
    draw_footer(l, f, 3, 4);
}

fn draw_page4(l: &PdfLayerReference,
              f: &IndirectFontRef, fb: &IndirectFontRef,
              report: &TrainingReport) {
    page_subheader(l, fb, "ACCURACY CURVE (CHECKPOINT)");
    let mut y = PH - 25.0;

    match report {
        TrainingReport::RandomForest { n_trees, train_accuracy, val_accuracy, training_secs,
                                       accuracy_curve, .. } => {
            y = section_title(l, fb, &format!("Random Forest Summary -- {} Trees", n_trees), y);
            y -= 2.0;
            let time_str = format_training_time(*training_secs as f32);
            let summary = [
                ("Training Accuracy",   format!("{:.6}", train_accuracy)),
                ("Testing Accuracy", format!("{:.6}", val_accuracy)),
                ("Total Trees",         format!("{}", n_trees)),
                ("Training Time",       time_str),
            ];
            y = kv_table(l, f, fb, &summary, y);
            y -= 6.0;
            
            accuracy_checkpoint_table(l, f, fb, "N Trees", accuracy_curve, y);
        }

        TrainingReport::MLP { n_epochs, train_loss, val_loss, training_secs: _,
                              accuracy_curve, .. } => {
            y = section_title(l, fb, &format!("MLP Summary -- {} Epochs", n_epochs), y);
            y -= 2.0;
            let summary = [
                ("Train Loss",      format!("{:.6}", train_loss)),
                ("Testing Loss", format!("{:.6}", val_loss)),
                ("Total Epochs",    format!("{}", n_epochs)),
            ];
            y = kv_table(l, f, fb, &summary, y);
            y -= 6.0;
            accuracy_checkpoint_table(l, f, fb, "Epoch", accuracy_curve, y);
        }

        TrainingReport::SVM { n_epochs, n_support_vectors, final_loss, training_secs: _,
                              accuracy_curve, .. } => {
            y = section_title(l, fb, &format!("SVM Summary -- {} Epochs", n_epochs), y);
            y -= 2.0;
            let summary = [
                ("Train Loss",      format!("{:.6}", final_loss)),
                ("Support Vectors", format!("{}", n_support_vectors)),
                ("Total Epochs",    format!("{}", n_epochs)),
            ];
            y = kv_table(l, f, fb, &summary, y);
            y -= 6.0;

            accuracy_checkpoint_table(l, f, fb, "Epoch", accuracy_curve, y);
        }

        TrainingReport::LSTM { n_epochs, train_loss, val_loss, training_secs: _,
                               accuracy_curve, .. } => {
            y = section_title(l, fb, &format!("LSTM Summary -- {} Epochs", n_epochs), y);
            y -= 2.0;
            let summary = [
                ("Train Loss",      format!("{:.6}", train_loss)),
                ("Testing Loss", format!("{:.6}", val_loss)),
                ("Total Epochs",    format!("{}", n_epochs)),
            ];
            y = kv_table(l, f, fb, &summary, y);
            y -= 6.0;
            accuracy_checkpoint_table(l, f, fb, "Epoch", accuracy_curve, y);
        }

        TrainingReport::CNN { n_epochs, train_loss, val_loss, training_secs: _,
                              accuracy_curve, .. } => {
            y = section_title(l, fb, &format!("1D-CNN Summary -- {} Epochs", n_epochs), y);
            y -= 2.0;
            let summary = [
                ("Train Loss",      format!("{:.6}", train_loss)),
                ("Testing Loss", format!("{:.6}", val_loss)),
                ("Total Epochs",    format!("{}", n_epochs)),
            ];
            y = kv_table(l, f, fb, &summary, y);
            y -= 6.0;
            accuracy_checkpoint_table(l, f, fb, "Epoch", accuracy_curve, y);
        }
    }

    draw_footer(l, f, 4, 5);
}

fn draw_page5(l: &PdfLayerReference,
              f: &IndirectFontRef, fb: &IndirectFontRef,
              report: &TrainingReport) {
    page_subheader(l, fb, "LOSS CURVE (CHECKPOINT)");
    let y = PH - 25.0;

    match report {
        TrainingReport::RandomForest { .. } => {
            // RF tidak punya loss curve
            txt_center(l, "Random Forest - No Loss Curve", 11.0, PH / 2.0, f, c_dark_gray());
        }

        TrainingReport::MLP { loss_curve, .. } => {
            loss_checkpoint_table(l, f, fb, loss_curve, y);
        }

        TrainingReport::SVM { .. } => {
            // SVM tidak punya loss curve tracking
            txt_center(l, "SVM - No Loss Curve", 11.0, PH / 2.0, f, c_dark_gray());
        }

        TrainingReport::LSTM { loss_curve, .. } => {
            loss_checkpoint_table(l, f, fb, loss_curve, y);
        }

        TrainingReport::CNN { loss_curve, .. } => {
            loss_checkpoint_table(l, f, fb, loss_curve, y);
        }
    }

    draw_footer(l, f, 5, 5);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Generate a structured 5-page A4 PDF training report.
/// Returns the output file path on success, or an error message on failure.
/// Satu titik epoch untuk kurva per-fold.
pub struct EpochPoint {
    pub epoch: usize,
    pub train_acc: f32,
    pub val_acc: f32,
    pub train_loss: f32,
    pub val_loss: f32,
}

/// Data satu fold LOOO untuk PDF.
pub struct LooFold {
    pub origin: String,
    pub test_acc: f32,
    pub train_acc: f32,
    pub cm: [[usize; 2]; 2],       // confusion matrix test fold
    pub train_cm: [[usize; 2]; 2], // confusion matrix train fold
    pub curve: Vec<EpochPoint>,    // kurva per-epoch fold ini
    pub final_train_loss: f32,
    pub final_val_loss: f32,
    pub secs: f64, // waktu fold ini (detik)
}

/// Bootstrap 95% CI untuk accuracy, precision, recall, F1 dari CONFUSION MATRIX
/// AGREGAT (240 prediksi, 2 kelas). Ini lebih benar daripada CI per-fold LOOO yang
/// tiap fold cuma 1 kelas (membuat precision selalu = 1.0, menyesatkan).
/// Konvensi: cm[t][p], kelas 1=high (positif), 0=low. Mengembalikan untuk tiap
/// metrik: (mean, lo, hi).
fn bootstrap_ci_confusion(cm: &[[usize; 4]; 4]) -> [(f32, f32, f32); 4] {
    let mut samples: Vec<(u8, u8)> = Vec::new();
    for t in 0..4 {
        for p in 0..4 {
            for _ in 0..cm[t][p] {
                samples.push((t as u8, p as u8));
            }
        }
    }
    let n = samples.len();
    if n == 0 {
        return [(0.0, 0.0, 0.0); 4];
    }

    let metrics_of = |idx: &[usize]| -> (f32, f32, f32, f32) {
        let mut tcm = [[0usize; 4]; 4];
        for &i in idx {
            let (yt, yp) = samples[i];
            tcm[yt as usize][yp as usize] += 1;
        }
        metrics_from_cm(&tcm)
    };

    // Nilai titik (point estimate) dari seluruh data.
    let all_idx: Vec<usize> = (0..n).collect();
    let (acc0, prec0, rec0, f10) = metrics_of(&all_idx);

    // RNG deterministik (xorshift64) → reproducible.
    let mut state: u64 = 0xD1B54A32D192ED03;
    let mut next_index = |bound: usize| -> usize {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % bound as u64) as usize
    };

    let n_boot = 5000usize;
    let mut accs = Vec::with_capacity(n_boot);
    let mut precs = Vec::with_capacity(n_boot);
    let mut recs = Vec::with_capacity(n_boot);
    let mut f1s = Vec::with_capacity(n_boot);
    let mut idx_buf = vec![0usize; n];
    for _ in 0..n_boot {
        for k in 0..n {
            idx_buf[k] = next_index(n);
        }
        let (a, p, r, f) = metrics_of(&idx_buf);
        accs.push(a);
        precs.push(p);
        recs.push(r);
        f1s.push(f);
    }

    let ci = |v: &mut Vec<f32>, point: f32| -> (f32, f32, f32) {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let pick = |p: f32| v[((p * (n_boot as f32 - 1.0)).round() as usize).min(n_boot - 1)];
        (point, pick(0.025), pick(0.975))
    };

    [
        ci(&mut accs, acc0),
        ci(&mut precs, prec0),
        ci(&mut recs, rec0),
        ci(&mut f1s, f10),
    ]
}

/// Hitung AUC (area under ROC) via statistik Mann-Whitney U / rank.
/// scores = p_high, labels: 1=high (positif), 0=low (negatif).
fn auc_from_scores(scores: &[f32], labels: &[i64]) -> f32 {
    let n = scores.len();
    if n == 0 { return 0.5; }
    // pasangkan lalu urutkan berdasarkan skor untuk ranking (rata-rata rank utk ties)
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| scores[a].partial_cmp(&scores[b]).unwrap_or(std::cmp::Ordering::Equal));
    let mut ranks = vec![0.0f64; n];
    let mut i = 0usize;
    while i < n {
        let mut j = i + 1;
        while j < n && (scores[idx[j]] - scores[idx[i]]).abs() < f32::EPSILON {
            j += 1;
        }
        // rank rata-rata utk grup ties pada indeks i..j
        let avg_rank = ((i + 1 + j) as f64) / 2.0; // rank 1-based
        for k in i..j {
            ranks[idx[k]] = avg_rank;
        }
        i = j;
    }
    let n_pos = labels.iter().filter(|&&l| l == 1).count();
    let n_neg = n - n_pos;
    if n_pos == 0 || n_neg == 0 {
        return 0.5; // tak terdefinisi → netral
    }
    let sum_ranks_pos: f64 = (0..n).filter(|&k| labels[k] == 1).map(|k| ranks[k]).sum();
    let auc = (sum_ranks_pos - (n_pos as f64) * (n_pos as f64 + 1.0) / 2.0)
        / ((n_pos as f64) * (n_neg as f64));
    auc as f32
}

/// Bootstrap 95% CI untuk AUC dari skor+label per-sampel (deterministik, reproducible).
/// Mengembalikan (auc_point, lo, hi).
fn bootstrap_ci_auc(scores: &[f32], labels: &[i64]) -> (f32, f32, f32) {
    let n = scores.len();
    if n == 0 { return (0.5, 0.5, 0.5); }
    let point = auc_from_scores(scores, labels);

    let mut state: u64 = 0xA0761D6478BD642F;
    let mut next_index = |bound: usize| -> usize {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % bound as u64) as usize
    };

    let n_boot = 5000usize;
    let mut aucs: Vec<f32> = Vec::with_capacity(n_boot);
    let mut bs = vec![0.0f32; n];
    let mut bl = vec![0i64; n];
    for _ in 0..n_boot {
        for k in 0..n {
            let r = next_index(n);
            bs[k] = scores[r];
            bl[k] = labels[r];
        }
        aucs.push(auc_from_scores(&bs, &bl));
    }
    aucs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pick = |p: f32| aucs[((p * (n_boot as f32 - 1.0)).round() as usize).min(n_boot - 1)];
    (point, pick(0.025), pick(0.975))
}

#[allow(dead_code)]
/// Bootstrap 95% confidence interval untuk rata-rata sekumpulan nilai per-fold.
/// Deterministik (seed tetap) supaya angka CI bisa direproduksi untuk publikasi.
/// Mengembalikan (mean, ci_lower, ci_upper) pada level 95% (percentile 2.5 & 97.5).
fn bootstrap_ci_95(values: &[f32]) -> (f32, f32, f32) {
    let n = values.len();
    if n == 0 {
        return (0.0, 0.0, 0.0);
    }
    let mean = values.iter().sum::<f32>() / n as f32;
    if n < 2 {
        return (mean, mean, mean);
    }

    // RNG deterministik (xorshift64) dengan seed tetap → hasil reproducible.
    let mut state: u64 = 0x9E3779B97F4A7C15;
    let mut next_index = |bound: usize| -> usize {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % bound as u64) as usize
    };

    let n_boot = 5000usize;
    let mut boot_means: Vec<f32> = Vec::with_capacity(n_boot);
    for _ in 0..n_boot {
        let mut s = 0.0f32;
        for _ in 0..n {
            let idx = next_index(n);
            s += values[idx];
        }
        boot_means.push(s / n as f32);
    }
    boot_means.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let pct = |p: f32| -> f32 {
        let rank = (p * (n_boot as f32 - 1.0)).round() as usize;
        boot_means[rank.min(n_boot - 1)]
    };
    (mean, pct(0.025), pct(0.975))
}

/// Generate PDF ringkasan LOOO lengkap: summary, evaluation agregat, tabel 12 fold,
/// ROC/AUC, lalu detail per-fold (train + test confusion matrix).
pub fn generate_loo_summary_pdf(
    model_name: &str,
    folds: &[LooFold],
    roc: &LooRoc,
    agg_eval: &EvaluationResults,
    total_secs: f64,
    output_dir: &str,
) -> Result<String, String> {
    let confusion = agg_eval.confusion_matrix;
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let model_tag = model_name.trim().replace(' ', "_").to_uppercase();
    let path = format!("{}/LOOO_{}_{}.pdf", output_dir, model_tag, ts);

    // Hitung total halaman: 1 summary + 1 aggregate-CM + mean-curve + 1 ROC + per-fold
    let max_ep_global = folds.iter().map(|fd| fd.curve.len()).max().unwrap_or(0);
    let mean_curve_pages = ((max_ep_global + 49) / 50).max(1) as u32;
    let total_pages: u32 = 1 + 1 + mean_curve_pages + 1 + folds.iter().map(|fd| {
        let ep_pages = ((fd.curve.len() + 49) / 50).max(1) as u32;
        1 + ep_pages
    }).sum::<u32>();

    let (doc, p1, l1) = PdfDocument::new(
        format!("Leave-One-Origin-Out {} Report", model_name),
        Mm(PW), Mm(PH), "Layer 1");
    let f  = doc.add_builtin_font(BuiltinFont::Helvetica).map_err(|e| e.to_string())?;
    let fb = doc.add_builtin_font(BuiltinFont::HelveticaBold).map_err(|e| e.to_string())?;

    // ── Halaman 1: judul + tabel 12 fold + mean±std ──
    let l = doc.get_page(p1).get_layer(l1);
    fill_rect(&l, 0.0, PH, PW, 18.0, c_dark_green());
    txt(&l, "LEAVE-ONE-ORIGIN-OUT Testing REPORT", 14.0, ML, PH - 12.0, &fb, c_white());
    let mut y = PH - 28.0;
    txt(&l, &format!("Model: {}", model_name), 10.0, ML, y, &fb, c_black()); y -= 6.0;
    txt(&l, &format!("Number of folds: {}", folds.len()), 9.0, ML, y, &f, c_dark_gray()); y -= 5.0;
    txt(&l, &format!("Total time: {}", format_training_time(total_secs as f32)), 9.0, ML, y, &f, c_dark_gray()); y -= 12.0;

    // ── RINGKASAN: rata-rata train/val acc + loss + F1 agregat ──
    let nf = folds.len().max(1) as f32;
    let mean_train_acc = folds.iter().map(|fd| fd.train_acc).sum::<f32>() / nf;
    let mean_test_acc  = folds.iter().map(|fd| fd.test_acc).sum::<f32>() / nf;
    let mean_train_loss = folds.iter().map(|fd| fd.final_train_loss).sum::<f32>() / nf;
    let mean_val_loss   = folds.iter().map(|fd| fd.final_val_loss).sum::<f32>() / nf;

    y = section_title(&l, &fb, "Training & Testing Summary (mean over 12 folds)", y);
    y -= 2.0;
    let sum_headers = ["Metric", "Value"];
    let sum_widths = [90.0f32, 90.0];
    let sum_rows = vec![
        vec!["Mean Train Accuracy".to_string(), format!("{:.2}%", mean_train_acc * 100.0)],
        vec!["Mean Testing Accuracy".to_string(), format!("{:.2}%", mean_test_acc * 100.0)],
        vec!["Mean Train Loss".to_string(), format!("{:.4}", mean_train_loss)],
        vec!["Mean Testing Loss".to_string(), format!("{:.4}", mean_val_loss)],
        vec!["AUC (aggregate)".to_string(), format!("{:.4}", roc.auc)],
    ];
    y = ranked_table(&l, &f, &fb, &sum_headers, &sum_widths, &sum_rows, None, y);
    y -= 10.0;

    y = section_title(&l, &fb, "Per-Fold Results (each origin as test set)", y);
    y -= 2.0;

    // Per-fold metrics dihitung dari confusion matrix fold (test).
    // Konvensi high=1, low=0: cm[1][1]=TP_high, cm[0][0]=TN_low.
    let fold_metrics = |cm: &[[usize; 2]; 2]| -> (f32, f32, f32) {
        // Untuk fold 1 kelas, hitung accuracy + precision/recall/f1 kelas yang ada.
        let tp_h = cm[1][1] as f32; let fn_h = cm[1][0] as f32; let fp_h = cm[0][1] as f32;
        let tp_l = cm[0][0] as f32; let fn_l = cm[0][1] as f32; let fp_l = cm[1][0] as f32;
        // kelas yang ada (yang punya support)
        let has_high = (cm[1][0] + cm[1][1]) > 0;
        let (prec, rec) = if has_high {
            let p = if tp_h + fp_h > 0.0 { tp_h / (tp_h + fp_h) } else { 0.0 };
            let r = if tp_h + fn_h > 0.0 { tp_h / (tp_h + fn_h) } else { 0.0 };
            (p, r)
        } else {
            let p = if tp_l + fp_l > 0.0 { tp_l / (tp_l + fp_l) } else { 0.0 };
            let r = if tp_l + fn_l > 0.0 { tp_l / (tp_l + fn_l) } else { 0.0 };
            (p, r)
        };
        let f1 = if prec + rec > 0.0 { 2.0 * prec * rec / (prec + rec) } else { 0.0 };
        (prec, rec, f1)
    };

    // Tabel gabungan: Origin, Class, Test Acc, Train Acc, Precision, Recall, F1
    let headers = ["Origin (Test)", "Class", "Test Acc", "Train Acc", "Precision", "Recall", "F1"];
    let widths = [54.0f32, 18.0, 24.0, 24.0, 22.0, 20.0, 18.0];
    let mut rows: Vec<Vec<String>> = folds.iter().map(|fd| {
        let class = if fd.origin.starts_with("high") { "High" } else { "Low" };
        let (p, r, f1) = fold_metrics(&fd.cm);
        vec![
            fd.origin.clone(),
            class.to_string(),
            format!("{:.4}", fd.test_acc),
            format!("{:.4}", fd.train_acc),
            format!("{:.4}", p),
            format!("{:.4}", r),
            format!("{:.4}", f1),
        ]
    }).collect();

    // mean & std
    let n = folds.len().max(1) as f32;
    let mean = |sel: &dyn Fn(&LooFold) -> f32| -> f32 { folds.iter().map(sel).sum::<f32>() / n };
    let std = |sel: &dyn Fn(&LooFold) -> f32, m: f32| -> f32 {
        if folds.len() < 2 { return 0.0; }
        (folds.iter().map(|fd| (sel(fd) - m).powi(2)).sum::<f32>() / n).sqrt()
    };
    let m_test = mean(&|fd| fd.test_acc);   let s_test = std(&|fd| fd.test_acc, m_test);
    let m_tr   = mean(&|fd| fd.train_acc);  let s_tr   = std(&|fd| fd.train_acc, m_tr);
    let m_p = mean(&|fd| fold_metrics(&fd.cm).0);
    let m_r = mean(&|fd| fold_metrics(&fd.cm).1);
    let m_f = mean(&|fd| fold_metrics(&fd.cm).2);

    rows.push(vec![
        "MEAN +/- STD".to_string(),
        "-".to_string(),
        format!("{:.3}+/-{:.3}", m_test, s_test),
        format!("{:.3}+/-{:.3}", m_tr, s_tr),
        format!("{:.3}", m_p),
        format!("{:.3}", m_r),
        format!("{:.3}", m_f),
    ]);

    y = ranked_table(&l, &f, &fb, &headers, &widths, &rows, None, y);
    y -= 8.0;

    // ── Confidence Intervals (95%, bootstrap dari 240 prediksi agregat) ──
    // CI dihitung dari confusion matrix AGREGAT (2 kelas), bukan per-fold LOOO.
    // Alasan: tiap fold LOOO hanya berisi 1 kelas, sehingga precision/recall per-fold
    // menyesatkan (precision selalu 1.0). Agregat 240 prediksi memberi CI yang benar.
    let ci_metrics = bootstrap_ci_confusion(&confusion);
    let (acc_m, acc_lo, acc_hi)   = ci_metrics[0];
    let (prec_m, prec_lo, prec_hi) = ci_metrics[1];
    let (rec_m, rec_lo, rec_hi)   = ci_metrics[2];
    let (f1_m, f1_lo, f1_hi)      = ci_metrics[3];

    // CI AUC dari skor p_high per-sampel (bootstrap). Kalau skor tak tersedia,
    // pakai nilai agregat tanpa CI.
    let auc_row = if roc.scores.len() >= 2 && roc.labels.len() == roc.scores.len() {
        let (auc_m, auc_lo, auc_hi) = bootstrap_ci_auc(&roc.scores, &roc.labels);
        vec!["AUC".to_string(), format!("{:.4}", auc_m), format!("{:.4}", auc_lo), format!("{:.4}", auc_hi)]
    } else {
        vec!["AUC (aggregate)".to_string(), format!("{:.4}", roc.auc), "-".to_string(), "-".to_string()]
    };

    y = section_title(&l, &fb,
        "95% Confidence Intervals (bootstrap, 240 aggregate predictions, 5000 resamples)", y);
    y -= 2.0;
    let ci_headers = ["Metric", "Estimate", "95% CI (lower)", "95% CI (upper)"];
    let ci_widths = [54.0f32, 40.0, 44.0, 44.0];
    let ci_rows = vec![
        vec!["Accuracy".to_string(),  format!("{:.4}", acc_m),  format!("{:.4}", acc_lo),  format!("{:.4}", acc_hi)],
        vec!["Precision".to_string(), format!("{:.4}", prec_m), format!("{:.4}", prec_lo), format!("{:.4}", prec_hi)],
        vec!["Recall".to_string(),    format!("{:.4}", rec_m),  format!("{:.4}", rec_lo),  format!("{:.4}", rec_hi)],
        vec!["F1-Score".to_string(),  format!("{:.4}", f1_m),   format!("{:.4}", f1_lo),   format!("{:.4}", f1_hi)],
        auc_row,
    ];
    y = ranked_table(&l, &f, &fb, &ci_headers, &ci_widths, &ci_rows, None, y);
    y -= 4.0;
    txt(&l, "Bootstrap nonparametrik (95%, percentile 2.5-97.5). Acc/Prec/Rec/F1 atas 240 prediksi; AUC atas skor p_high per-sampel.",
        7.0, ML, y, &f, c_dark_gray());
    y -= 8.0;

    // ── Global metrics from aggregate 240 predictions (2 classes, valid) ──
    y = section_title(&l, &fb, "Global Metrics (aggregate of 240 predictions, 2 classes)", y);
    y -= 2.0;
    let gh = &agg_eval.high_grade_metrics;
    let gl = &agg_eval.low_grade_metrics;
    let g_headers = ["Metric", "High Grade", "Low Grade"];
    let g_widths = [60.0f32, 60.0, 60.0];
    let g_rows = vec![
        vec!["Precision".to_string(), format!("{:.4}", gh.precision), format!("{:.4}", gl.precision)],
        vec!["Recall".to_string(),    format!("{:.4}", gh.recall),    format!("{:.4}", gl.recall)],
        vec!["F1-Score".to_string(),  format!("{:.4}", gh.f1_score),  format!("{:.4}", gl.f1_score)],
        vec!["Support".to_string(),   format!("{}", gh.support),      format!("{}", gl.support)],
    ];
    y = ranked_table(&l, &f, &fb, &g_headers, &g_widths, &g_rows, None, y);
    let _ = y;

    draw_footer(&l, &f, 1, total_pages);

    // ── Halaman 1c: Aggregate Confusion Matrix (halaman sendiri agar muat penuh) ──
    {
        let (pc, lc) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
        let lcr = doc.get_page(pc).get_layer(lc);
        fill_rect(&lcr, 0.0, PH, PW, 18.0, c_dark_green());
        txt(&lcr, "AGGREGATE CONFUSION MATRIX", 13.0, ML, PH - 12.0, &fb, c_white());
        let mut cy = PH - 30.0;
        txt(&lcr, &format!("Combined predictions of all {} LOOO folds (240 predictions, 2 classes).", folds.len()),
            9.0, ML, cy, &f, c_dark_gray());
        cy -= 10.0;
        let _ = confusion_matrix_grid(&lcr, &f, &fb, confusion,
            Some("Confusion Matrix (Aggregate Testing)"), cy);
        draw_footer(&lcr, &f, 2, total_pages);
    }

    // ── Halaman 1b: Mean Per-Epoch Curve (rata-rata semua fold) ──
    {
        // Hitung rata-rata train/val acc & loss per epoch across folds
        let max_ep = folds.iter().map(|fd| fd.curve.len()).max().unwrap_or(0);
        let mut mean_rows: Vec<Vec<String>> = Vec::new();
        for e in 0..max_ep {
            let mut ta = 0.0; let mut va = 0.0; let mut tl = 0.0; let mut vl = 0.0; let mut cnt = 0.0;
            for fd in folds {
                if e < fd.curve.len() {
                    ta += fd.curve[e].train_acc; va += fd.curve[e].val_acc;
                    tl += fd.curve[e].train_loss; vl += fd.curve[e].val_loss; cnt += 1.0;
                }
            }
            if cnt > 0.0 {
                mean_rows.push(vec![
                    format!("{}", e + 1),
                    format!("{:.2}", ta / cnt * 100.0),
                    format!("{:.2}", va / cnt * 100.0),
                    format!("{:.4}", tl / cnt),
                    format!("{:.4}", vl / cnt),
                ]);
            }
        }
        let mep_headers = ["Epoch", "Mean Train Acc (%)", "Mean Val Acc (%)", "Mean Train Loss", "Mean Val Loss"];
        let mep_widths = [28.0f32, 42.0, 42.0, 38.0, 38.0];
        // Pecah 50/halaman
        let chunks: Vec<&[Vec<String>]> = mean_rows.chunks(50).collect();
        let nch = chunks.len().max(1);
        for (ci, chunk) in chunks.iter().enumerate() {
            let (pm, lm) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
            let lmr = doc.get_page(pm).get_layer(lm);
            fill_rect(&lmr, 0.0, PH, PW, 18.0, c_dark_green());
            txt(&lmr, &format!("MEAN PER-EPOCH CURVE (all 12 folds) ({}/{})", ci + 1, nch), 12.0, ML, PH - 12.0, &fb, c_white());
            let mut my = PH - 28.0;
            txt(&lmr, "Average of train/Testing accuracy and loss per epoch across all folds.", 9.0, ML, my, &f, c_dark_gray());
            my -= 8.0;
            let _ = compact_epoch_table(&lmr, &f, &fb, &mep_headers, &mep_widths, chunk, my);
            draw_footer(&lmr, &f, 3 + ci as u32, total_pages);
        }
    }

    // ── Halaman ROC/AUC ──
    let (p2, l2) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    let l2r = doc.get_page(p2).get_layer(l2);
    draw_roc_page(&l2r, &f, &fb, roc);
    draw_footer(&l2r, &f, 3 + mean_curve_pages, total_pages);

    // ── Halaman per-fold (2 halaman tiap fold: confusion + tabel epoch) ──
    let mut page_no = 4 + mean_curve_pages;
    for (idx, fd) in folds.iter().enumerate() {
        // === Halaman A: confusion matrix train + test ===
        let (pp, ll) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
        let lr = doc.get_page(pp).get_layer(ll);
        fill_rect(&lr, 0.0, PH, PW, 18.0, c_dark_green());
        txt(&lr, &format!("FOLD {}/{} - CONFUSION MATRIX", idx + 1, folds.len()), 13.0, ML, PH - 12.0, &fb, c_white());

        let mut yy = PH - 30.0;
        let kelas = if fd.origin.starts_with("high") { "High Grade" } else { "Low Grade" };
        txt(&lr, &format!("Test Origin: {}  (class: {}, time: {:.1}s)", fd.origin, kelas, fd.secs), 11.0, ML, yy, &fb, c_black()); yy -= 10.0;

        yy = section_title(&lr, &fb, &format!("Training Data (11 origins) - Accuracy: {:.2}%", fd.train_acc * 100.0), yy);
        yy -= 3.0;
        yy = confusion_matrix_grid(&lr, &f, &fb, fd.train_cm, None, yy);
        yy -= 10.0;

        yy = section_title(&lr, &fb, &format!("Testing Data ({}) - Accuracy: {:.2}%", fd.origin, fd.test_acc * 100.0), yy);
        yy -= 3.0;
        yy = confusion_matrix_grid(&lr, &f, &fb, fd.cm, None, yy);
        yy -= 8.0;

        txt(&lr, "Note: the test origin contains only one class, so the other class row is empty.", 8.5, ML, yy, &f, c_dark_gray()); yy -= 4.5;
        txt(&lr, "This is expected in LOOO. Training uses 11 origins (both classes present).", 8.5, ML, yy, &f, c_dark_gray());
        draw_footer(&lr, &f, page_no, total_pages);
        page_no += 1;

        // === Halaman B (bisa multi-halaman): tabel per-epoch ===
        let ep_headers = ["Epoch", "Train Acc (%)", "Val Acc (%)", "Train Loss", "Val Loss"];
        let ep_widths = [30.0f32, 40.0, 40.0, 35.0, 35.0];
        let ep_rows: Vec<Vec<String>> = fd.curve.iter().map(|e| vec![
            format!("{}", e.epoch),
            format!("{:.2}", e.train_acc * 100.0),
            format!("{:.2}", e.val_acc * 100.0),
            format!("{:.4}", e.train_loss),
            format!("{:.4}", e.val_loss),
        ]).collect();

        // Pecah ke beberapa halaman (50 baris/halaman, compact)
        let per_page = 50usize;
        let chunks: Vec<&[Vec<String>]> = ep_rows.chunks(per_page).collect();
        let n_chunks = chunks.len().max(1);
        for (ci, chunk) in chunks.iter().enumerate() {
            let (pe, le) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
            let ler = doc.get_page(pe).get_layer(le);
            fill_rect(&ler, 0.0, PH, PW, 18.0, c_dark_green());
            txt(&ler, &format!("FOLD {}/{} - PER-EPOCH TABLE ({}/{})", idx + 1, folds.len(), ci + 1, n_chunks), 12.0, ML, PH - 12.0, &fb, c_white());
            let mut ey = PH - 28.0;
            txt(&ler, &format!("Origin: {}  |  Final Train Loss: {:.4}  |  Final Val Loss: {:.4}",
                fd.origin, fd.final_train_loss, fd.final_val_loss), 9.0, ML, ey, &f, c_dark_gray());
            ey -= 8.0;
            let _ = compact_epoch_table(&ler, &f, &fb, &ep_headers, &ep_widths, chunk, ey);
            draw_footer(&ler, &f, page_no, total_pages);
            page_no += 1;
        }
    }

    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    doc.save(&mut BufWriter::new(File::create(&path).map_err(|e| e.to_string())?))
        .map_err(|e| e.to_string())?;
    Ok(path)
}

/// Titik ROC untuk PDF (fpr, tpr).
pub struct LooRoc {
    pub points: Vec<(f32, f32)>, // (fpr, tpr)
    pub auc: f32,
    pub n_origins: usize,
    pub n_high: usize,
    pub n_low: usize,
    /// Skor p_high per-sampel (240) + label asli, untuk bootstrap CI AUC.
    pub scores: Vec<f32>,
    pub labels: Vec<i64>,
}

/// Generate PDF LOOO: sama seperti report training + 1 halaman ROC/AUC agregat.
pub fn generate_loo_pdf(report: &TrainingReport, roc: &LooRoc, output_dir: &str)
    -> Result<String, String>
{
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = format!("{}/LOOO_{}_{}.pdf", output_dir, report.model_short().to_uppercase(), ts);

    let (doc, p1, l1) = PdfDocument::new(
        format!("Leave-One-Origin-Out {} Report", report.model_name()),
        Mm(PW), Mm(PH), "Layer 1",
    );
    let f  = doc.add_builtin_font(BuiltinFont::Helvetica)    .map_err(|e| e.to_string())?;
    let fb = doc.add_builtin_font(BuiltinFont::HelveticaBold) .map_err(|e| e.to_string())?;

    draw_page1(&doc.get_page(p1).get_layer(l1), &f, &fb, report, ts);
    let (p2, l2) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    draw_page2(&doc.get_page(p2).get_layer(l2), &f, &fb, report);
    let (p3, l3) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    draw_page3(&doc.get_page(p3).get_layer(l3), &f, &fb, report);
    let (p4, l4) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    draw_page4(&doc.get_page(p4).get_layer(l4), &f, &fb, report);
    let (p5, l5) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    draw_page5(&doc.get_page(p5).get_layer(l5), &f, &fb, report);

    // Halaman ekstra: ROC & AUC
    let (p6, l6) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    draw_roc_page(&doc.get_page(p6).get_layer(l6), &f, &fb, roc);

    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    doc.save(&mut BufWriter::new(
        File::create(&path).map_err(|e| e.to_string())?
    )).map_err(|e| e.to_string())?;

    Ok(path)
}

/// Gambar halaman ROC curve + AUC.
fn draw_roc_page(l: &PdfLayerReference, f: &IndirectFontRef, fb: &IndirectFontRef, roc: &LooRoc) {
    let mut y = PH - ML;
    y = section_title(l, fb, "ROC Curve & AUC (Leave-One-Origin-Out, Aggregate)", y);
    y -= 6.0;

    let interp = if roc.auc >= 0.90 { "Excellent" }
        else if roc.auc >= 0.80 { "Very Good" }
        else if roc.auc >= 0.70 { "Good" }
        else if roc.auc >= 0.60 { "Fair" }
        else if roc.auc >= 0.50 { "Poor" }
        else { "No discrimination" };

    txt(l, &format!("AUC = {:.4}  ({})", roc.auc, interp), 12.0, ML, y, fb, c_dark_green()); y -= 6.0;
    txt(l, &format!("Samples: {} high / {} low   |   {} origins", roc.n_high, roc.n_low, roc.n_origins), 9.0, ML, y, f, c_dark_gray());
    y -= 12.0;

    // ── Plot geometry: persegi, sisakan ruang utk legend di kanan ──
    // plot_x diberi ruang 16mm di kiri untuk label sumbu-Y; legend mulai setelah plot.
    let plot_x = ML + 16.0;          // 31 mm
    let plot_w = 112.0f32;           // kanan plot = 143 mm
    let plot_h = 112.0f32;
    let plot_y = y - plot_h;

    // Background terang (fill_rect memakai y sebagai tepi ATAS, gambar ke bawah)
    fill_rect(l, plot_x, plot_y + plot_h, plot_w, plot_h, Color::Rgb(Rgb::new(0.97, 0.98, 1.0, None)));

    // Grid lines (10 garis) — dibatasi di dalam kotak plot
    l.set_outline_color(Color::Rgb(Rgb::new(0.85, 0.88, 0.92, None)));
    l.set_outline_thickness(0.3);
    for i in 1..10 {
        let gx = plot_x + (i as f32 / 10.0) * plot_w;
        l.add_line(Line { points: vec![
            (Point::new(Mm(gx), Mm(plot_y)), false),
            (Point::new(Mm(gx), Mm(plot_y + plot_h)), false)], is_closed: false });
        let gy = plot_y + (i as f32 / 10.0) * plot_h;
        l.add_line(Line { points: vec![
            (Point::new(Mm(plot_x), Mm(gy)), false),
            (Point::new(Mm(plot_x + plot_w), Mm(gy)), false)], is_closed: false });
    }

    // Area terisi di bawah kurva ROC (poligon) — koordinat di-clamp ke [0,1]
    if roc.points.len() >= 2 {
        let mut poly: Vec<(Point, bool)> = Vec::new();
        poly.push((Point::new(Mm(plot_x), Mm(plot_y)), false)); // (0,0)
        for (fpr, tpr) in &roc.points {
            let fx = fpr.clamp(0.0, 1.0);
            let ty = tpr.clamp(0.0, 1.0);
            poly.push((Point::new(
                Mm(plot_x + fx * plot_w),
                Mm(plot_y + ty * plot_h)), false));
        }
        poly.push((Point::new(Mm(plot_x + plot_w), Mm(plot_y)), false)); // (1,0)
        l.set_fill_color(Color::Rgb(Rgb::new(0.80, 0.88, 0.98, None)));
        l.add_polygon(Polygon {
            rings: vec![poly],
            mode: PaintMode::Fill,
            winding_order: WindingOrder::NonZero,
        });
    }

    // Garis diagonal (random classifier)
    l.set_outline_color(c_mid_gray());
    l.set_outline_thickness(0.6);
    l.add_line(Line { points: vec![
        (Point::new(Mm(plot_x), Mm(plot_y)), false),
        (Point::new(Mm(plot_x + plot_w), Mm(plot_y + plot_h)), false)], is_closed: false });

    // Kurva ROC (biru tebal)
    if roc.points.len() >= 2 {
        l.set_outline_color(Color::Rgb(Rgb::new(0.0, 0.30, 0.75, None)));
        l.set_outline_thickness(1.6);
        let pts: Vec<(Point, bool)> = roc.points.iter()
            .map(|(fpr, tpr)| (Point::new(
                Mm(plot_x + fpr.clamp(0.0, 1.0) * plot_w),
                Mm(plot_y + tpr.clamp(0.0, 1.0) * plot_h)), false))
            .collect();
        l.add_line(Line { points: pts, is_closed: false });
    }

    // Kotak plot (digambar terakhir agar di atas grid & fill); y = tepi atas
    stroke_rect(l, plot_x, plot_y + plot_h, plot_w, plot_h, c_dark_gray(), 0.6);

    // Tick label sumbu (0, 0.5, 1.0)
    for i in 0..=2 {
        let frac = i as f32 / 2.0;
        txt(l, &format!("{:.1}", frac), 7.0, plot_x + frac * plot_w - 3.0, plot_y - 5.0, f, c_dark_gray());
        txt_right(l, &format!("{:.1}", frac), 7.0, plot_x - 3.0, plot_y + frac * plot_h - 1.0, f, c_dark_gray());
    }

    // Label sumbu-X (di bawah plot, ditengahkan terhadap lebar plot)
    let xlab = "False Positive Rate (1 - Specificity)";
    txt(l, xlab, 9.0, plot_x + (plot_w - aw(xlab, 9.0)) / 2.0, plot_y - 11.0, fb, c_black());

    // Label sumbu-Y (vertikal, dirotasi 90 derajat, di kiri plot)
    draw_vertical_text(l, "True Positive Rate (Sensitivity)", 9.0,
                       plot_x - 9.0, plot_y + plot_h / 2.0 - 30.0, fb, c_black());

    // AUC label di area kosong kanan-bawah dalam plot
    let auc_lab = format!("AUC = {:.3}", roc.auc);
    txt(l, &auc_lab, 12.0, plot_x + plot_w * 0.52, plot_y + plot_h * 0.18, fb, c_dark_green());

    // ── Legend interpretasi di kanan plot (muat dalam margin halaman) ──
    let lx = plot_x + plot_w + 8.0;         // ~151 mm
    let mut ly = plot_y + plot_h - 2.0;
    txt(l, "AUC Interpretation", 9.5, lx, ly, fb, c_dark_green()); ly -= 8.0;
    for (rng, lab) in [("0.90-1.00", "Excellent"), ("0.80-0.90", "Very Good"),
                       ("0.70-0.80", "Good"), ("0.60-0.70", "Fair"),
                       ("0.50-0.60", "Poor"), ("< 0.50", "Random")] {
        txt(l, &format!("{}", lab), 8.0, lx, ly, fb, c_dark_gray());
        txt(l, &format!("{}", rng), 7.0, lx, ly - 3.8, f, c_mid_gray());
        ly -= 11.0;
    }
    // Penanda kurva
    ly -= 2.0;
    l.set_outline_color(Color::Rgb(Rgb::new(0.0, 0.30, 0.75, None)));
    l.set_outline_thickness(1.6);
    l.add_line(Line { points: vec![
        (Point::new(Mm(lx), Mm(ly + 1.2)), false),
        (Point::new(Mm(lx + 8.0), Mm(ly + 1.2)), false)], is_closed: false });
    txt(l, "ROC curve", 7.5, lx + 10.0, ly, f, c_dark_gray()); ly -= 6.0;
    l.set_outline_color(c_mid_gray());
    l.set_outline_thickness(0.6);
    l.add_line(Line { points: vec![
        (Point::new(Mm(lx), Mm(ly + 1.2)), false),
        (Point::new(Mm(lx + 8.0), Mm(ly + 1.2)), false)], is_closed: false });
    txt(l, "Random", 7.5, lx + 10.0, ly, f, c_dark_gray());

    // Keterangan bawah
    let mut ty = plot_y - 20.0;
    txt(l, "Note: ROC/AUC is computed from the combined predictions of all LOOO folds (240 predictions),", 8.5, ML, ty, f, c_dark_gray());
    ty -= 5.0;
    txt(l, "because each fold tests a single origin (one class), so AUC cannot be computed per fold.", 8.5, ML, ty, f, c_dark_gray());
}

pub fn generate_training_pdf(report: &TrainingReport, output_dir: &str)
    -> Result<String, String>
{
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = format!("{}/{}_report_{}.pdf", output_dir, report.model_short(), ts);

    let (doc, p1, l1) = PdfDocument::new(
        format!("{} Training Report", report.model_name()),
        Mm(PW), Mm(PH), "Layer 1",
    );
    let f  = doc.add_builtin_font(BuiltinFont::Helvetica)    .map_err(|e| e.to_string())?;
    let fb = doc.add_builtin_font(BuiltinFont::HelveticaBold) .map_err(|e| e.to_string())?;

    draw_page1(&doc.get_page(p1).get_layer(l1), &f, &fb, report, ts);

    let (p2, l2) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    draw_page2(&doc.get_page(p2).get_layer(l2), &f, &fb, report);

    let (p3, l3) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    draw_page3(&doc.get_page(p3).get_layer(l3), &f, &fb, report);

    let (p4, l4) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    draw_page4(&doc.get_page(p4).get_layer(l4), &f, &fb, report);

    let (p5, l5) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    draw_page5(&doc.get_page(p5).get_layer(l5), &f, &fb, report);

    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    doc.save(&mut BufWriter::new(
        File::create(&path).map_err(|e| e.to_string())?
    )).map_err(|e| e.to_string())?;

    Ok(path)
}
// ════════════════════════════════════════════════════════════════════════
//  INDEPENDENT BATCH TEST REPORT (Assessment)
//  Train: model full (12 origin). Test: sampel independen di luar origin.
//  Metrik dihitung per-FILE (mis. 100 file = 5 sampel x 20). Plus ringkasan
//  per-sampel (majority vote). Bukti generalisasi ke batch baru utk reviewer.
// ════════════════════════════════════════════════════════════════════════

/// Ringkasan hasil 1 sampel independen (gabungan 20 file via majority vote).
#[derive(Clone)]
pub struct IndepSample {
    pub name: String,        // mis. "HIGH_Bali_Kintamani"
    pub true_label: i64,     // 1=high, 0=low (dari prefix folder)
    pub n_files: usize,      // jumlah file diuji
    pub n_high: usize,       // berapa file diprediksi high
    pub n_low: usize,        // berapa file diprediksi low
    pub pred_label: i64,     // hasil majority vote (1/0)
    pub confidence: f32,     // rata-rata prob kelas pemenang
    pub secs: f64,           // waktu evaluasi sampel ini (detik)
}

/// Data lengkap independent test untuk PDF.
pub struct IndepReport {
    pub model_name: String,
    pub samples: Vec<IndepSample>,
    pub confusion: [[usize; 4]; 4], // per-FILE agregat (t][p], 1=high positif)
    pub scores: Vec<f32>,           // p_high per-file (utk AUC + CI)
    pub labels: Vec<i64>,           // label asli per-file
    pub total_secs: f64,
}

/// Hitung accuracy/precision/recall/f1 dari confusion matrix (positif=high=1).
fn metrics_from_cm(cm: &[[usize; 4]; 4]) -> (f32, f32, f32, f32) {
    let mut tp = [0.0_f32; 4];
    let mut fp = [0.0_f32; 4];
    let mut fn_ = [0.0_f32; 4];
    let mut total = 0.0_f32;
    let mut correct = 0.0_f32;

    for i in 0..4 {
        for j in 0..4 {
            let val = cm[i][j] as f32;
            total += val;
            if i == j {
                tp[i] += val;
                correct += val;
            } else {
                fn_[i] += val;
                fp[j] += val;
            }
        }
    }

    let acc = if total > 0.0 { correct / total } else { 0.0 };

    let mut macro_prec = 0.0;
    let mut macro_rec = 0.0;
    let mut macro_f1 = 0.0;

    for i in 0..4 {
        let p = if tp[i] + fp[i] > 0.0 { tp[i] / (tp[i] + fp[i]) } else { 0.0 };
        let r = if tp[i] + fn_[i] > 0.0 { tp[i] / (tp[i] + fn_[i]) } else { 0.0 };
        let f = if p + r > 0.0 { 2.0 * p * r / (p + r) } else { 0.0 };
        macro_prec += p;
        macro_rec += r;
        macro_f1 += f;
    }

    (acc, macro_prec / 4.0, macro_rec / 4.0, macro_f1 / 4.0)
}

/// Generate PDF Independent Batch Assessment.
pub fn generate_independent_pdf(rep: &IndepReport, output_dir: &str) -> Result<String, String> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let model_tag = rep.model_name.trim().replace(' ', "_").to_uppercase();
    let path = format!("{}/CROSSDAY_{}_{}.pdf", output_dir, model_tag, ts);
    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;

    let (doc, p1, l1) = PdfDocument::new(
        format!("Cross Day Assessment - {}", rep.model_name),
        Mm(PW), Mm(PH), "Layer 1");
    let f  = doc.add_builtin_font(BuiltinFont::Helvetica).map_err(|e| e.to_string())?;
    let fb = doc.add_builtin_font(BuiltinFont::HelveticaBold).map_err(|e| e.to_string())?;

    let l = doc.get_page(p1).get_layer(l1);
    fill_rect(&l, 0.0, PH, PW, 18.0, c_dark_green());
    txt(&l, "CROSS DAY ASSESSMENT", 14.0, ML, PH - 12.0, &fb, c_white());
    let mut y = PH - 28.0;
    txt(&l, &format!("Model: {}", rep.model_name), 10.0, ML, y, &fb, c_black()); y -= 6.0;
    txt(&l, "Train: full model (all 12 origins). Test: cross day/independent coffee batches.",
        8.0, ML, y, &f, c_dark_gray()); y -= 5.0;
    let n_files: usize = rep.samples.iter().map(|s| s.n_files).sum();
    txt(&l, &format!("Cross-day samples: {}  |  Total files tested: {}  |  Time: {:.1}s",
        rep.samples.len(), n_files, rep.total_secs), 8.0, ML, y, &f, c_dark_gray());
    y -= 10.0;

    // ── Tabel per-sampel (5 baris) ──
    y = section_title(&l, &fb, "Per-sample results (per-file prediction)", y);
    y -= 2.0;
    let s_headers = ["Sample", "True Label", "Predicted", "Total", "Confidence", "Time"];
    let s_widths = [62.0f32, 28.0, 28.0, 20.0, 24.0, 26.0];
    let mut s_rows: Vec<Vec<String>> = Vec::new();
    for s in &rep.samples {
        let class_names = ["Baseline", "Normal", "Overheating", "Arcing"];
        let tl = class_names.get(s.true_label as usize).unwrap_or(&"Unknown");
        let pl = class_names.get(s.pred_label as usize).unwrap_or(&"Unknown");
        s_rows.push(vec![
            s.name.clone(),
            tl.to_string(),
            pl.to_string(),
            format!("{}", s.n_files),
            format!("{:.1}%", s.confidence * 100.0),
            format!("{:.1}s", s.secs),
        ]);
    }
    y = ranked_table(&l, &f, &fb, &s_headers, &s_widths, &s_rows, None, y);
    y -= 10.0;

    // ── Confusion matrix per-FILE (styled, colored, seperti LOOO) ──
    let cm = rep.confusion;
    let _ = confusion_matrix_grid(&l, &f, &fb, cm,
        Some(&format!("Confusion Matrix (per file, n={})", n_files)), y);

    // ── Metrik + 95% CI (bootstrap) ── (halaman baru utk metrik agar rapi)
    let (pm, pml) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
    let l = doc.get_page(pm).get_layer(pml);
    fill_rect(&l, 0.0, PH, PW, 18.0, c_dark_green());
    txt(&l, "EVALUATION METRICS & SUMMARY", 14.0, ML, PH - 12.0, &fb, c_white());
    let mut y = PH - 32.0;
    let (acc, prec, rec, f1) = metrics_from_cm(&cm);
    let ci = bootstrap_ci_confusion(&cm);
    let auc_point = if rep.scores.len() >= 2 && rep.labels.len() == rep.scores.len() {
        Some(bootstrap_ci_auc(&rep.scores, &rep.labels))
    } else { None };
    let auc_row = match auc_point {
        Some((am, lo, hi)) => vec!["AUC".to_string(), format!("{:.4}", am), format!("{:.4}", lo), format!("{:.4}", hi)],
        None => vec!["AUC".to_string(), "-".to_string(), "-".to_string(), "-".to_string()],
    };

    y = section_title(&l, &fb, "Metrics with 95% CI (bootstrap per-file, 5000 resamples)", y);
    y -= 2.0;
    let m_headers = ["Metric", "Estimate", "95% CI (lower)", "95% CI (upper)"];
    let m_widths = [54.0f32, 40.0, 44.0, 44.0];
    let m_rows = vec![
        vec!["Accuracy".to_string(),         format!("{:.4}", acc),  format!("{:.4}", ci[0].1), format!("{:.4}", ci[0].2)],
        vec!["Precision".to_string(), format!("{:.4}", prec), format!("{:.4}", ci[1].1), format!("{:.4}", ci[1].2)],
        vec!["Recall".to_string(),    format!("{:.4}", rec),  format!("{:.4}", ci[2].1), format!("{:.4}", ci[2].2)],
        vec!["F1-Score".to_string(),  format!("{:.4}", f1),   format!("{:.4}", ci[3].1), format!("{:.4}", ci[3].2)],
        auc_row,
    ];
    y = ranked_table(&l, &f, &fb, &m_headers, &m_widths, &m_rows, None, y);
    y -= 8.0;

    // ── Summary / interpretation (jelas, seperti reviewer mau) ──
    y = section_title(&l, &fb, "Summary", y);
    y -= 2.0;
    let total_correct: usize = (0..4).map(|i| cm[i][i]).sum();
    let total_files: usize = rep.samples.iter().map(|s| s.n_files).sum();
    let auc_txt = match auc_point {
        Some((am, _, _)) => format!("{:.4}", am),
        None => "n/a".to_string(),
    };
    let lines = [
        format!("Files correctly classified: {}/{} ({:.1}%).",
            total_correct, total_files,
            if total_files > 0 { total_correct as f32 / total_files as f32 * 100.0 } else { 0.0 }),
        format!("Accuracy {:.1}%, precision {:.1}%, recall {:.1}%, F1 {:.1}%, AUC {}.", acc*100.0, prec*100.0, rec*100.0, f1*100.0, auc_txt),
        "Model trained on all 12 origins; evaluated on coffee batches outside the training set.".to_string(),
    ];
    for line in &lines {
        txt(&l, line, 8.5, ML, y, &f, c_black());
        y -= 6.0;
    }

    doc.save(&mut BufWriter::new(File::create(&path).map_err(|e| e.to_string())?))
        .map_err(|e| e.to_string())?;
    Ok(path)
}