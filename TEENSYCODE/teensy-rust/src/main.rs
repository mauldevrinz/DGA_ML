#![no_std]
#![no_main]

use teensy4_bsp as bsp;
use teensy4_panic as _;

use bsp::board;
use bsp::hal::flexpwm;
use bsp::interrupt;
use bsp::hal::timer::Blocking;
use bsp::hal::usbd::{BusAdapter, EndpointMemory, EndpointState, Speed};
use core::cell::RefCell;
use core::fmt::Write as _;
use cortex_m::interrupt::Mutex;
use embedded_hal::blocking::i2c::Write as _;
use embedded_hal::digital::v2::OutputPin;
use usb_device::bus::UsbBusAllocator;
use usb_device::device::{UsbDevice, UsbDeviceBuilder, UsbDeviceState, UsbVidPid};
use usb_device::UsbError;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

use ads1x1x::{Ads1x1x, ChannelSelection, SlaveAddr, FullScaleRange, DynamicOneShot};

// ===================================================
// INA226 - fungsi baca/tulis register (generik, bisa dipakai untuk
// proxy shared-bus apa pun yang mengimplementasikan trait i2c terkait)
// ===================================================
fn ina226_write<I2C>(i2c: &mut I2C, addr: u8, reg: u8, value: u16)
where
    I2C: embedded_hal::blocking::i2c::Write,
{
    let _ = i2c.write(addr, &[reg, (value >> 8) as u8, (value & 0xFF) as u8]);
}

fn ina226_read<I2C>(i2c: &mut I2C, addr: u8, reg: u8) -> u16
where
    I2C: embedded_hal::blocking::i2c::WriteRead,
{
    let mut buf = [0u8; 2];
    let _ = i2c.write_read(addr, &[reg], &mut buf);
    ((buf[0] as u16) << 8) | (buf[1] as u16)
}

// ===================================================
// Peltier / BTS7960 - konversi duty cycle (0..=1000 permil) menjadi nilai
// compare "turn off" untuk FlexPWM. Counter submodule dijalankan penuh dari
// i16::MIN sampai i16::MAX (lihat init di main), dan pulsa PWM aktif mulai
// dari i16::MIN (turn_on tetap) sampai nilai balikan fungsi ini (turn_off).
// ===================================================
fn pwm_permille_to_turn_off(permille: u16) -> i16 {
    let permille = permille.min(1000) as i32;
    let span = u16::MAX as i32 + 1; // 65536, lebar penuh counter i16
    let value = i16::MIN as i32 + (span * permille) / 1000;
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

// ===================================================
// USB CDC serial dua-arah. Menggantikan bsp::LoggingFrontend (yang cuma
// satu arah, Teensy -> PC) supaya firmware bisa menerima perintah dari
// host, misalnya "SETPOINT=25.5" untuk mengubah target suhu Peltier.
// ===================================================

    pub enum HostCmd {
        Setpoint(f32),
        Phase(&'static str),
        Pump1Pwm(u16),
        Pwr(i16),
    }

struct UsbComm<'a> {
    device: UsbDevice<'a, BusAdapter>,
    serial: SerialPort<'a, BusAdapter>,
    configured: bool,
}

impl<'a> UsbComm<'a> {
    /// Jalankan mesin state USB. Harus dipanggil berkala (setiap ada
    /// tulis/baca, dan idealnya juga tiap iterasi loop) supaya host tetap
    /// menganggap device hidup dan transfer data benar-benar jalan.
    fn poll(&mut self) {
        if self.device.poll(&mut [&mut self.serial])
            && !self.configured
            && self.device.state() == UsbDeviceState::Configured
        {
            // Wajib dipanggil sekali setelah device pertama kali Configured,
            // sesuai requirement driver imxrt-usbd.
            self.device.bus().configure();
            self.configured = true;
        }
    }

    /// Kirim satu baris teks + newline. Tidak memblokir tanpa batas kalau
    /// tidak ada host yang membaca (mis. belum dicolok ke PC) - setiap
    /// gagal-tulis tetap memanggil poll() lalu retry, dan proses lain di
    /// firmware (sensor, kontrol Peltier) tidak menunggu ini selesai.
    fn write_line(&mut self, line: &str) {
        self.poll();
        let mut bytes = line.as_bytes();
        while !bytes.is_empty() {
            match self.serial.write(bytes) {
                Ok(0) => self.poll(),
                Ok(n) => bytes = &bytes[n..],
                Err(UsbError::WouldBlock) => self.poll(),
                Err(_) => break,
            }
        }
        let mut nl: &[u8] = b"\r\n";
        while !nl.is_empty() {
            match self.serial.write(nl) {
                Ok(0) => self.poll(),
                Ok(n) => nl = &nl[n..],
                Err(UsbError::WouldBlock) => self.poll(),
                Err(_) => break,
            }
        }
    }

    /// Baca perintah dari host secara non-blocking. Satu-satunya perintah
    /// yang dikenali saat ini: baris "SETPOINT=<angka>" (mis. "SETPOINT=25.5"),
    /// dikirim oleh tombol setpoint suhu di web app. Mengembalikan nilai
    /// baru kalau baris SETPOINT valid baru saja selesai diterima.
    fn poll_cmd(&mut self, rx_buf: &mut [u8], rx_len: &mut usize) -> Option<HostCmd> {
        self.poll();
        let mut chunk = [0u8; 64];
        let n = self.serial.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return None;
        }
        let mut result: Option<HostCmd> = None;
        for &b in &chunk[..n] {
            if b == b'\n' || b == b'\r' {
                if *rx_len > 0 {
                    if let Ok(line) = core::str::from_utf8(&rx_buf[..*rx_len]) {
                        let line = line.trim();
                        let parsed = if let Some(rest) = line.strip_prefix("SETPOINT=") {
                            rest.trim().parse::<f32>().ok().map(HostCmd::Setpoint)
                        } else if let Some(rest) = line.strip_prefix("PHASE=") {
                            match rest.trim() {
                                "IDLE" => Some(HostCmd::Phase("IDLE")),
                                "INJECT" => Some(HostCmd::Phase("INJECT")),
                                "PURGE" => Some(HostCmd::Phase("PURGE")),
                                "OFF" => Some(HostCmd::Phase("OFF")),
                                _ => None,
                            }
                        } else if let Some(rest) = line.strip_prefix("PWM=") {
                            rest.trim().parse::<u16>().ok().map(HostCmd::Pump1Pwm)
                        } else if let Some(rest) = line.strip_prefix("PWR=") {
                            rest.trim().parse::<i16>().ok().map(HostCmd::Pwr)
                        } else {
                            None
                        };
                        if parsed.is_some() {
                            result = parsed;
                        }
                    }
                }
                *rx_len = 0;
            } else if *rx_len < rx_buf.len() {
                rx_buf[*rx_len] = b;
                *rx_len += 1;
            }
        }
        result
    }
}

// ===================================================
// USB diservis dari interrupt (USB_OTG1), bukan cuma polling manual dari
// application code. Ini pola yang sama dipakai bsp::LoggingFrontend (yang
// dulu dipakai di sini dan terbukti stabil) - polling manual dari main()
// saja ternyata tidak cukup responsif terhadap SETUP packet pertama dari
// host, menyebabkan Windows gagal membaca device descriptor sepenuhnya
// ("Invalid Device Descriptor" / VID:PID kosong di Device Manager/usbipd).
// ===================================================
static USB_COMM: Mutex<RefCell<Option<UsbComm<'static>>>> = Mutex::new(RefCell::new(None));

fn with_usb_comm<R>(f: impl FnOnce(&mut UsbComm<'static>) -> R) -> Option<R> {
    cortex_m::interrupt::free(|cs| USB_COMM.borrow(cs).borrow_mut().as_mut().map(f))
}

macro_rules! process_usb_commands {
    ($rx_buf:expr, $rx_len:expr, $setpoint:expr, $phase:expr, $pump1_pwm:expr, $pwr_mode:expr, $pwr_val:expr, $set_pump_fn:expr, $p41:expr, $p39:expr, $p40:expr, $p14:expr, $p38:expr) => {
        while let Some(cmd) = with_usb_comm(|comm| comm.poll_cmd($rx_buf, $rx_len)).flatten() {
            match cmd {
                HostCmd::Setpoint(val) => {
                    $setpoint = val;
                    $pwr_mode = false;
                    usb_println!("ACK SETPOINT = {:.2} C", $setpoint);
                },
                HostCmd::Phase(p) => {
                    $phase = p;
                    usb_println!("ACK PHASE = {}", p);

                    // Handle Phases immediately
                    match $phase {
                        "IDLE" => {
                            let _ = $p41.set_high(); // Pump 3 ON
                            let _ = $p39.set_high(); // SV2
                            let _ = $p40.set_high(); // SV3
                            let _ = $p14.set_high(); // Pump 2 ON
                            let _ = $p38.set_low(); // SV1
                            $set_pump_fn(0);
                        },
                        "INJECT" => {
                            $set_pump_fn($pump1_pwm); // Pump 1
                            let _ = $p38.set_high(); // SV1
                            let _ = $p14.set_low(); // Pump 2
                            let _ = $p41.set_low(); // Pump 3
                            let _ = $p40.set_low(); // SV3
                            let _ = $p39.set_low(); // SV2
                        },
                        "PURGE" => {
                            let _ = $p41.set_high(); // Pump 3 ON
                            let _ = $p39.set_high(); // SV2
                            let _ = $p40.set_high(); // SV3
                            let _ = $p38.set_low(); // SV1
                            let _ = $p14.set_high(); // Pump 2 ON
                            $set_pump_fn(0);
                        },
                        _ => { // OFF
                            let _ = $p14.set_low();
                            let _ = $p38.set_low();
                            let _ = $p39.set_low();
                            let _ = $p40.set_low();
                            let _ = $p41.set_low();
                            $set_pump_fn(0);
                        }
                    }
                },
                HostCmd::Pump1Pwm(val) => {
                    let val_permille = (val.min(100) as u16) * 10;
                    $pump1_pwm = val_permille;
                    usb_println!("ACK PWM = {}%", val.min(100));
                    if $phase == "INJECT" {
                        $set_pump_fn($pump1_pwm);
                    }
                },
                HostCmd::Pwr(val) => {
                    $pwr_mode = true;
                    $pwr_val = val;
                    usb_println!("ACK PWR = {}", val);
                }
            }
        }
    }
}

#[bsp::rt::interrupt]
fn USB_OTG1() {
    with_usb_comm(|comm| comm.poll());
}

/// Buffer baris tetap (tanpa alokasi heap, cocok untuk no_std) dipakai
/// untuk memformat teks sebelum dikirim lewat `UsbComm::write_line`.
struct LineBuf {
    buf: [u8; 128],
    len: usize,
}

impl LineBuf {
    fn new() -> Self {
        Self { buf: [0; 128], len: 0 }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

impl core::fmt::Write for LineBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buf.len() - self.len;
        let n = bytes.len().min(remaining);
        self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
        Ok(())
    }
}

/// Pengganti `log::info!` lama: format ke `LineBuf` lalu kirim lewat
/// `UsbComm`. Dipakai persis seperti `log::info!(...)` sebelumnya.
macro_rules! usb_println {
    ($($arg:tt)*) => {{
        let mut line = LineBuf::new();
        let _ = write!(line, $($arg)*);
        with_usb_comm(|comm| comm.write_line(line.as_str()));
    }};
}

// ===================================================
// NDIR SENSOR — SGX IR12EM inline processing
// Ported from HydroCarbon-ELKA/hello-world (RTIC) → polling loop.
// Signal chain: ADS1115 raw → Median5 → EMA → NdirState → ProcessedFilter
// ===================================================

// -- Tuning constants (adjusted for ADS1115 16-bit, 0..32767 single-ended) --
const NDIR_RAW_EMA_ALPHA:  f32 = 0.25;    // EMA weight for raw ADC channels
const NDIR_PROC_EMA_ALPHA: f32 = 0.20;    // EMA weight for ratio/response/absorbance
const NDIR_REF_DIFF_MIN:   u16 = 500;     // min ref_diff for valid ratio (~1.5% of 32767)
const NDIR_HALF_PERIOD_MS: u32 = 125;     // 4 Hz lamp, 50% duty → 125 ms half-period
const NDIR_BASELINE_CYCLES: u32 = 80;     // pairs for baseline (~80 s at 1 pair/s loop)

const MOX_EMA_ALPHA:       f32 = 0.20;    // EMA weight for slow MOX sensors

/// Single-channel EMA: y[n] = α·x[n] + (1-α)·y[n-1]
#[derive(Clone, Copy)]
struct NdirEma { alpha: f32, value: f32, initialized: bool }

impl NdirEma {
    const fn new(alpha: f32) -> Self {
        Self { alpha, value: 0.0, initialized: false }
    }
    fn update(&mut self, x: f32) -> f32 {
        if !self.initialized {
            self.value = x;
            self.initialized = true;
        } else {
            self.value = self.alpha * x + (1.0 - self.alpha) * self.value;
        }
        self.value
    }
    fn update_i16(&mut self, x: i16) -> i16 {
        self.update(x as f32) as i16
    }
    fn as_u16(&self) -> u16 {
        self.value.max(0.0).min(u16::MAX as f32) as u16
    }
}

/// Median-of-5 sliding window denoiser.
struct NdirMedian5 { buf: [u16; 5], idx: usize, filled: u8 }

impl NdirMedian5 {
    const fn new() -> Self {
        Self { buf: [0; 5], idx: 0, filled: 0 }
    }
    fn update(&mut self, x: u16) -> u16 {
        self.buf[self.idx] = x;
        self.idx = (self.idx + 1) % 5;
        if self.filled < 5 { self.filled += 1; }
        let n = self.filled as usize;
        let mut s = [0u16; 5];
        s[..n].copy_from_slice(&self.buf[..n]);
        // Insertion sort on ≤5 elements — trivially fast.
        for i in 1..n {
            let key = s[i];
            let mut j = i;
            while j > 0 && s[j - 1] > key { s[j] = s[j - 1]; j -= 1; }
            s[j] = key;
        }
        s[n / 2]
    }
}

/// Combined per-channel filter: Median5 → EMA.
struct NdirChannelFilter { median: NdirMedian5, ema: NdirEma }

impl NdirChannelFilter {
    const fn new(alpha: f32) -> Self {
        Self { median: NdirMedian5::new(), ema: NdirEma::new(alpha) }
    }
    /// Push raw ADC count. Returns (median_u16, ema_f32).
    fn update(&mut self, raw: u16) -> (u16, f32) {
        let med = self.median.update(raw);
        let ema = self.ema.update(med as f32);
        (med, ema)
    }
    /// EMA output rounded to u16.
    fn filt_u16(&self) -> u16 { self.ema.as_u16() }
}

/// ON half-cycle raw sample.
#[derive(Clone, Copy, Default)]
struct NdirHalfSample { act: u16, reference: u16 }

/// Values produced after one complete ON/OFF lamp pair.
#[derive(Clone, Copy, Default)]
struct NdirProcessedSample {
    _act_diff:       u16,
    _ref_diff:       u16,
    ratio:          f32,
    baseline_ratio: f32,
    response_pct:   f32,
    baseline_ready: bool,
}

/// NDIR state machine — pure math, no HAL.
struct NdirState {
    on_sample:       Option<NdirHalfSample>,
    baseline_sum:    f32,
    baseline_count:  u32,
    baseline_target: u32,
    baseline_ratio:  f32,
    baseline_ready:  bool,
}

impl NdirState {
    const fn new(baseline_target: u32) -> Self {
        Self {
            on_sample: None, baseline_sum: 0.0, baseline_count: 0,
            baseline_target, baseline_ratio: 1.0, baseline_ready: false,
        }
    }

    /// Store ON-phase sample (filtered u16 values).
    fn capture_on(&mut self, act: u16, reference: u16) {
        self.on_sample = Some(NdirHalfSample { act, reference });
    }

    /// Store OFF-phase sample, compute and return the processed pair.
    /// Returns None if no ON sample captured or ref_diff < NDIR_REF_DIFF_MIN.
    fn capture_off_and_process(
        &mut self, act_off: u16, ref_off: u16,
    ) -> Option<NdirProcessedSample> {
        let on = self.on_sample.take()?;
        let act_diff = on.act.abs_diff(act_off);
        let ref_diff = on.reference.abs_diff(ref_off);

        if ref_diff < NDIR_REF_DIFF_MIN { return None; }

        let ratio = act_diff as f32 / ref_diff as f32;

        if !self.baseline_ready {
            self.baseline_sum   += ratio;
            self.baseline_count += 1;
            if self.baseline_count >= self.baseline_target {
                self.baseline_ratio = self.baseline_sum / self.baseline_count as f32;
                self.baseline_ready = true;
            }
            return Some(NdirProcessedSample {
                _act_diff: act_diff, _ref_diff: ref_diff, ratio,
                baseline_ratio: self.baseline_ratio,
                response_pct: 0.0, baseline_ready: self.baseline_ready,
            });
        }

        let response_pct =
            ((ratio - self.baseline_ratio) / self.baseline_ratio) * 100.0;
        Some(NdirProcessedSample {
            _act_diff: act_diff, _ref_diff: ref_diff, ratio,
            baseline_ratio: self.baseline_ratio,
            response_pct, baseline_ready: true,
        })
    }
}

/// EMA filters for processed outputs (ratio, response_pct, absorbance).
struct NdirProcessedFilter {
    ratio_ema:      NdirEma,
    response_ema:   NdirEma,
    absorbance_ema: NdirEma,
}

/// Output of NdirProcessedFilter::update.
#[derive(Clone, Copy)]
struct NdirFilteredProcessed {
    ratio_filt:    f32,
    response_filt: f32,
    abs_raw:       f32,
    abs_filt:      f32,
}

impl NdirProcessedFilter {
    const fn new(alpha: f32) -> Self {
        Self {
            ratio_ema:      NdirEma::new(alpha),
            response_ema:   NdirEma::new(alpha),
            absorbance_ema: NdirEma::new(alpha),
        }
    }
    fn update(
        &mut self, ratio: f32, response_pct: f32, baseline_ratio: f32,
    ) -> NdirFilteredProcessed {
        let ratio_filt    = self.ratio_ema.update(ratio);
        let response_filt = self.response_ema.update(response_pct);
        let abs_raw       = ndir_absorbance(ratio, baseline_ratio);
        let abs_filt      = self.absorbance_ema.update(abs_raw);
        NdirFilteredProcessed { ratio_filt, response_filt, abs_raw, abs_filt }
    }
}

/// Beer-Lambert absorbance: A = −ln(ratio / baseline_ratio)
fn ndir_absorbance(ratio: f32, baseline_ratio: f32) -> f32 {
    if ratio <= 0.0 || baseline_ratio <= 0.0 { return 0.0; }
    -ndir_ln_approx(ratio / baseline_ratio)
}

/// Padé-1 ln approximation with range-reduction.
/// Accurate to ~1% for 0.5 < x < 2.0 (normal NDIR ratios).
fn ndir_ln_approx(x: f32) -> f32 {
    if x <= 0.0 { return 0.0; }
    let mut val = x;
    let mut adj = 0.0_f32;
    while val > 2.0 { val *= 0.5; adj += core::f32::consts::LN_2; }
    while val < 0.5 { val *= 2.0; adj -= core::f32::consts::LN_2; }
    let t = val - 1.0;
    2.0 * t / (val + 1.0) + adj
}

// ===================================================
// FUZZY PID CONTROLLER
// ===================================================
fn f32_min(a: f32, b: f32) -> f32 { if a < b { a } else { b } }
fn f32_max(a: f32, b: f32) -> f32 { if a > b { a } else { b } }
fn f32_clamp(v: f32, min: f32, max: f32) -> f32 { f32_min(f32_max(v, min), max) }

struct FuzzyPid {
    kp0: f32,
    ki0: f32,
    kd0: f32,
    integral: f32,
    last_error: f32,
}

impl FuzzyPid {
    fn new(kp0: f32, ki0: f32, kd0: f32) -> Self {
        Self {
            kp0, ki0, kd0,
            integral: 0.0,
            last_error: 0.0,
        }
    }

    fn trimf(x: f32, a: f32, b: f32, c: f32) -> f32 {
        if x <= a || x >= c {
            0.0
        } else if x <= b {
            (x - a) / (b - a)
        } else {
            (c - x) / (c - b)
        }
    }

    fn compute(&mut self, setpoint: f32, current: f32, dt_s: f32) -> i16 {
        let e = current - setpoint;
        let ec = if dt_s > 0.0 { (e - self.last_error) / dt_s } else { 0.0 };

        let e_norm = f32_clamp(e / 10.0, -1.0, 1.0);
        let ec_norm = f32_clamp(ec / 5.0, -1.0, 1.0);

        let mf_nb = |x: f32| Self::trimf(x, -2.0, -1.0, -0.5);
        let mf_ns = |x: f32| Self::trimf(x, -1.0, -0.5, 0.0);
        let mf_z  = |x: f32| Self::trimf(x, -0.5, 0.0, 0.5);
        let mf_ps = |x: f32| Self::trimf(x, 0.0, 0.5, 1.0);
        let mf_pb = |x: f32| Self::trimf(x, 0.5, 1.0, 2.0);

        let e_mfs = [mf_nb(e_norm), mf_ns(e_norm), mf_z(e_norm), mf_ps(e_norm), mf_pb(e_norm)];
        let ec_mfs = [mf_nb(ec_norm), mf_ns(ec_norm), mf_z(ec_norm), mf_ps(ec_norm), mf_pb(ec_norm)];

        let rules_kp = [
            [ 1.0,  1.0,  1.0,  0.5,  0.0],
            [ 1.0,  0.5,  0.5,  0.0, -0.5],
            [ 0.5,  0.0,  0.0,  0.0, -0.5],
            [-0.5,  0.0,  0.5,  0.5,  1.0],
            [ 0.0,  0.5,  1.0,  1.0,  1.0],
        ];

        let rules_ki = [
            [-1.0, -0.5,  0.0, -0.5, -1.0],
            [-0.5,  0.0,  0.5,  0.0, -0.5],
            [ 0.0,  0.5,  1.0,  0.5,  0.0],
            [-0.5,  0.0,  0.5,  0.0, -0.5],
            [-1.0, -0.5,  0.0, -0.5, -1.0],
        ];

        let rules_kd = [
            [ 1.0,  0.5,  0.0,  0.5,  1.0],
            [ 0.5,  0.0, -0.5,  0.0,  0.5],
            [ 0.0, -0.5, -1.0, -0.5,  0.0],
            [ 0.5,  0.0, -0.5,  0.0,  0.5],
            [ 1.0,  0.5,  0.0,  0.5,  1.0],
        ];

        let mut num_kp = 0.0;
        let mut num_ki = 0.0;
        let mut num_kd = 0.0;
        let mut den = 0.0;

        for i in 0..5 {
            for j in 0..5 {
                let weight = f32_min(e_mfs[i], ec_mfs[j]);
                den += weight;
                num_kp += weight * rules_kp[i][j];
                num_ki += weight * rules_ki[i][j];
                num_kd += weight * rules_kd[i][j];
            }
        }

        let (mut dkp, mut dki, mut dkd) = (0.0, 0.0, 0.0);
        if den > 0.0 {
            dkp = num_kp / den;
            dki = num_ki / den;
            dkd = num_kd / den;
        }

        let kp = f32_max(self.kp0 + dkp * 5.0, 0.0);
        let ki = f32_max(self.ki0 + dki * 0.5, 0.0);
        let kd = f32_max(self.kd0 + dkd * 1.0, 0.0);

        self.integral += e * dt_s;
        self.integral = f32_clamp(self.integral, -50.0, 50.0);

        let output = kp * e + ki * self.integral + kd * ec;
        self.last_error = e;

        f32_clamp(output * 10.0, -100.0, 100.0) as i16
    }
}

#[bsp::rt::entry]
fn main() -> ! {
    let board::Resources {
        pins,
        usb,
        pit,
        lpi2c1,
        lpi2c3,
        mut gpio1,
        mut gpio2,
        mut gpio3,
        mut gpio4,
        flexpwm1,
        flexpwm2,
        flexpwm4,
        ..
    } = bsp::board::t41(bsp::board::instances());

    let mut delay = Blocking::<_, { board::PERCLK_FREQUENCY }>::from_pit(pit.2);

    // ===================================================
    // USB CDC SERIAL (dua arah)
    // ===================================================
    static EP_MEMORY: EndpointMemory<1024> = EndpointMemory::new();
    static EP_STATE: EndpointState = EndpointState::max_endpoints();
    static mut USB_BUS: Option<UsbBusAllocator<BusAdapter>> = None;

    // Full-Speed (12 Mbit), bukan default High-Speed (480 Mbit) - HalfKay
    // bootloader bawaan chip (yang selalu berhasil enumerasi) juga cuma
    // Full-Speed. Kalau kabel/port kurang mendukung negosiasi High-Speed,
    // High-Speed bisa gagal total di awal enumerasi ("Invalid Device
    // Descriptor"), padahal Full-Speed jauh lebih toleran.
    let bus_adapter = BusAdapter::with_speed(usb, &EP_MEMORY, &EP_STATE, Speed::LowFull);
    // Perlu referensi 'static ke allocator supaya UsbDevice/SerialPort bisa
    // dipindah ke USB_COMM dan diakses dari interrupt USB_OTG1 di bawah.
    // Aman: hanya ditulis sekali di sini, tidak pernah dimutasi lagi setelahnya.
    #[allow(static_mut_refs)]
    let usb_bus_ref: &'static UsbBusAllocator<BusAdapter> = unsafe {
        USB_BUS = Some(UsbBusAllocator::new(bus_adapter));
        USB_BUS.as_ref().unwrap()
    };
    let serial = SerialPort::new(usb_bus_ref);
    let device = UsbDeviceBuilder::new(usb_bus_ref, UsbVidPid(0x16c0, 0x0483))
        .manufacturer("DGA")
        .product("Teensy DGA Acquisition")
        .serial_number("DGA-T41-01")
        .device_class(USB_CLASS_CDC)
        .build();

    // Aktifkan interrupt di level peripheral - tanpa ini hardware tidak
    // pernah men-trigger USB_OTG1 walau NVIC-nya di-unmask di bawah.
    device.bus().set_interrupts(true);

    cortex_m::interrupt::free(|cs| {
        *USB_COMM.borrow(cs).borrow_mut() = Some(UsbComm {
            device,
            serial,
            configured: false,
        });
    });

    // Paksa USB_OTG1 jalan sekali dulu supaya proses enumerasi mulai
    // dilayani interrupt sedini mungkin, baru unmask NVIC-nya.
    cortex_m::peripheral::NVIC::pend(interrupt::USB_OTG1);
    unsafe { cortex_m::peripheral::NVIC::unmask(interrupt::USB_OTG1) };

    // Tunggu device selesai enumerasi
    for _ in 0..3_000_000u32 {
        if with_usb_comm(|comm| comm.configured).unwrap_or(false) {
            break;
        }
    }

    for _ in 0..500 {
        delay.block_ms(1);
        with_usb_comm(|comm| comm.poll());
    }

    // ===================================================
    // GPIO INITIALIZATION
    // 15, 14, 41, 40, 39, 38
    // ===================================================
    // FAN (heatsink Peltier) - selalu menyala selama board hidup, tidak
    // pernah dimatikan oleh fase/mode manapun.
    let mut p15 = gpio1.output(pins.p15);
    let _ = p15.set_high();

    let mut p14 = gpio1.output(pins.p14);
    let _ = p14.set_low();

    let mut p41 = gpio1.output(pins.p41);
    let _ = p41.set_low();

    let mut p40 = gpio1.output(pins.p40);
    let _ = p40.set_low();

    let mut p39 = gpio1.output(pins.p39);
    let _ = p39.set_low();

    let mut p38 = gpio1.output(pins.p38);
    let _ = p38.set_low();

    // IR LAMP for NDIR sensor (IR12EM) — Teensy pin D22 / A8
    let mut ir_lamp = gpio1.output(pins.p22);
    let _ = ir_lamp.set_low(); // Lamp starts OFF


    // ===================================================
    // PUMP 1 - motor driver BTS7960
    // ===================================================
    let mut pump1_l_en = gpio4.output(pins.p5);
    let _ = pump1_l_en.set_low(); // Disabled at boot

    let mut pump1_r_en = gpio4.output(pins.p4);
    let _ = pump1_r_en.set_low(); // Disabled at boot

    let (mut pwm4, (_pwm4_sm0, _pwm4_sm1, mut pwm4_sm2, _pwm4_sm3)) = flexpwm4;
    pwm4_sm2.set_clock_select(flexpwm::ClockSelect::Ipg);
    pwm4_sm2.set_prescaler(flexpwm::Prescaler::Prescaler1);
    pwm4_sm2.set_pair_operation(flexpwm::PairOperation::Independent);
    pwm4_sm2.set_load_mode(flexpwm::LoadMode::reload_full());
    pwm4_sm2.set_load_frequency(1);
    pwm4_sm2.set_initial_count(&pwm4, i16::MIN);
    pwm4_sm2.set_value(flexpwm::FULL_RELOAD_VALUE_REGISTER, i16::MAX);

    let pump1_lpwm_out = flexpwm::Output::new_b(pins.p3);
    let pump1_rpwm_out = flexpwm::Output::new_a(pins.p2);
    pump1_lpwm_out.set_turn_on(&pwm4_sm2, i16::MIN);
    pump1_rpwm_out.set_turn_on(&pwm4_sm2, i16::MIN);

    pwm4_sm2.set_load_ok(&mut pwm4);
    pwm4_sm2.set_running(&mut pwm4, true);

    let mut set_pump1_pwm = |permille: u16| {
        // Swap: Matikan RPWM, jalankan LPWM untuk membalik arah putaran
        pump1_rpwm_out.set_output_enable(&mut pwm4, false);
        if permille == 0 {
            pump1_lpwm_out.set_output_enable(&mut pwm4, false);
            let _ = pump1_l_en.set_low(); // Disable bridge
            let _ = pump1_r_en.set_low();
        } else {
            let _ = pump1_l_en.set_high(); // Enable bridge
            let _ = pump1_r_en.set_high();
            pump1_lpwm_out.set_turn_off(&pwm4_sm2, pwm_permille_to_turn_off(permille));
            pwm4_sm2.set_load_ok(&mut pwm4);
            pump1_lpwm_out.set_output_enable(&mut pwm4, true);
        }
    };

    set_pump1_pwm(0);

    // ===================================================
    // PELTIER
// PELTIER - motor driver BTS7960 (half-bridge ganda)
    //   L_EN  -> D9   (enable jembatan kiri, digital)
    //   R_EN  -> D8   (enable jembatan kanan, digital)
    //   LPWM  -> D7   (PWM arah "kiri", FLEXPWM1 SM3 channel B)
    //   RPWM  -> D6   (PWM arah "kanan", FLEXPWM2 SM2 channel A)
    // Hanya salah satu dari LPWM/RPWM yang aktif (>0%) pada satu waktu -
    // itu yang menentukan arah arus ke Peltier (panas/dingin tergantung
    // polaritas pemasangan Peltier ke output OUT1/OUT2 modul BTS7960).
    // ===================================================
    let mut peltier_l_en = gpio2.output(pins.p9);
    let _ = peltier_l_en.set_low(); // Disabled at boot

    let mut peltier_r_en = gpio2.output(pins.p8);
    let _ = peltier_r_en.set_low(); // Disabled at boot

    let (mut pwm1, (_pwm1_sm0, _pwm1_sm1, _pwm1_sm2, mut pwm1_sm3)) = flexpwm1;
    let (mut pwm2, (_pwm2_sm0, _pwm2_sm1, mut pwm2_sm2, _pwm2_sm3)) = flexpwm2;

    // Konfigurasi submodule: clock IPG (150 MHz) tanpa prescaler, counter
    // dijalankan penuh dari i16::MIN..i16::MAX (~2.3 kHz), channel A/B
    // independen (bukan pasangan komplementer).
    pwm1_sm3.set_clock_select(flexpwm::ClockSelect::Ipg);
    pwm1_sm3.set_prescaler(flexpwm::Prescaler::Prescaler1);
    pwm1_sm3.set_pair_operation(flexpwm::PairOperation::Independent);
    pwm1_sm3.set_load_mode(flexpwm::LoadMode::reload_full());
    pwm1_sm3.set_load_frequency(1);
    pwm1_sm3.set_initial_count(&pwm1, i16::MIN);
    pwm1_sm3.set_value(flexpwm::FULL_RELOAD_VALUE_REGISTER, i16::MAX);

    pwm2_sm2.set_clock_select(flexpwm::ClockSelect::Ipg);
    pwm2_sm2.set_prescaler(flexpwm::Prescaler::Prescaler1);
    pwm2_sm2.set_pair_operation(flexpwm::PairOperation::Independent);
    pwm2_sm2.set_load_mode(flexpwm::LoadMode::reload_full());
    pwm2_sm2.set_load_frequency(1);
    pwm2_sm2.set_initial_count(&pwm2, i16::MIN);
    pwm2_sm2.set_value(flexpwm::FULL_RELOAD_VALUE_REGISTER, i16::MAX);

    let lpwm_out = flexpwm::Output::new_b(pins.p7);
    let rpwm_out = flexpwm::Output::new_a(pins.p6);
    lpwm_out.set_turn_on(&pwm1_sm3, i16::MIN);
    rpwm_out.set_turn_on(&pwm2_sm2, i16::MIN);

    pwm1_sm3.set_load_ok(&mut pwm1);
    pwm2_sm2.set_load_ok(&mut pwm2);
    pwm1_sm3.set_running(&mut pwm1, true);
    pwm2_sm2.set_running(&mut pwm2, true);

    // set_peltier_power(power): power dalam persen -100..=100.
    //   0        = Peltier mati (LPWM & RPWM off)
    //   1..100   = arah "kanan" (RPWM) sebesar |power|%
    //   -100..-1 = arah "kiri"  (LPWM) sebesar |power|%
    let mut set_peltier_power = |power: i16| {
        let power = power.clamp(-100, 100);
        let (left_pct, right_pct) = if power >= 0 {
            (0u16, power as u16)
        } else {
            ((-power) as u16, 0u16)
        };

        if left_pct == 0 && right_pct == 0 {
            // Power = 0: matikan semua, termasuk EN
            lpwm_out.set_output_enable(&mut pwm1, false);
            rpwm_out.set_output_enable(&mut pwm2, false);
            let _ = peltier_l_en.set_low(); // Disable bridge
            let _ = peltier_r_en.set_low();
        } else {
            // Power != 0: nyalakan EN dulu, baru set PWM
            let _ = peltier_l_en.set_high(); // Enable bridge
            let _ = peltier_r_en.set_high();

            if left_pct == 0 {
                lpwm_out.set_output_enable(&mut pwm1, false);
            } else {
                lpwm_out.set_turn_off(&pwm1_sm3, pwm_permille_to_turn_off(left_pct * 10));
                pwm1_sm3.set_load_ok(&mut pwm1);
                lpwm_out.set_output_enable(&mut pwm1, true);
            }

            if right_pct == 0 {
                rpwm_out.set_output_enable(&mut pwm2, false);
            } else {
                rpwm_out.set_turn_off(&pwm2_sm2, pwm_permille_to_turn_off(right_pct * 10));
                pwm2_sm2.set_load_ok(&mut pwm2);
                rpwm_out.set_output_enable(&mut pwm2, true);
            }
        }
    };

    // Pastikan Peltier mati saat boot, sampai kontrol otomatis di bawah
    // mengambil alih.
    set_peltier_power(0);

    // ===================================================
    // KONTROL PELTIER - bang-bang + hysteresis berbasis suhu chamber (SHT30)
    //   suhu > setpoint + hysteresis  -> mode COOL (polaritas "kanan"/RPWM,
    //                                    power positif)
    //   suhu < setpoint - hysteresis  -> mode HEAT (polaritas "kiri"/LPWM,
    //                                    power negatif)
    //   di antara keduanya            -> pertahankan mode sebelumnya
    //                                    (anti-chattering)
    //
    // Polaritas ini sudah dikonfirmasi terbalik dari asumsi awal (RPWM
    // ternyata mendinginkan, LPWM ternyata memanaskan) - kalau nanti
    // wiring Peltier ke OUT1/OUT2 BTS7960 diganti, cukup tukar lagi tanda
    // kedua konstanta power di bawah, tidak perlu ubah logika suhunya.
    // ===================================================
    let mut setpoint_c: f32 = 25.0;
    #[allow(unused_assignments)]
    let mut peltier_mode: &str = "OFF";
    let mut fuzzy_pid = FuzzyPid::new(5.0, 0.5, 1.0);

    let mut phase_mode: &str = "OFF";
    let mut pump1_pwm_val: u16 = 500; // default 50%

    let mut setpoint_rx_buf = [0u8; 48];
    let mut setpoint_rx_len: usize = 0;

    let mut manual_pwr_mode: bool = false;
    let mut manual_pwr_val: i16 = 0;

    macro_rules! my_usb_delay_ms {
        ($delay_ms:expr) => {{
            for _ in 0..$delay_ms {
                delay.block_ms(1);
                process_usb_commands!(
                    &mut setpoint_rx_buf, &mut setpoint_rx_len,
                    setpoint_c, phase_mode, pump1_pwm_val,
                    manual_pwr_mode, manual_pwr_val,
                    set_pump1_pwm, p41, p39, p40, p14, p38
                );
            }
        }};
    }

    // ===================================================
    // NDIR state machine + filters (IR12EM hydrocarbon sensor)
    // ===================================================
    let mut ndir = NdirState::new(NDIR_BASELINE_CYCLES);
    let mut ndir_act_cf = NdirChannelFilter::new(NDIR_RAW_EMA_ALPHA);
    let mut ndir_ref_cf = NdirChannelFilter::new(NDIR_RAW_EMA_ALPHA);
    let mut ndir_proc_filt = NdirProcessedFilter::new(NDIR_PROC_EMA_ALPHA);

    // ===================================================
    // EMA Filters for MOX Sensors (MQ & TGS series)
    // ===================================================
    // Index map:
    // ADS0: 0=mq2, 1=mq3, 2=mq4, 3=mq5
    // ADS1: 4=mq6, 5=mq7, 6=mq8, 7=mq135
    // ADS2: 8=tgs2600, 9=tgs2611, 10=tgs2610, 11=tgs822
    // ADS3: 12=tgs813, 13=mq9
    let mut mox_filters = [NdirEma::new(MOX_EMA_ALPHA); 14];

    // ===================================================
    // SYSTEM LED
    // RUN = 31 (gpio3), ERROR = 32 (gpio2)
    // ===================================================
    let mut led_run = gpio3.output(pins.p31);
    let _ = led_run.set_low();

    let mut led_error = gpio2.output(pins.p32);
    let _ = led_error.set_low();

    // ===================================================
    // I2C INITIALIZATION
    // SDA=18, SCL=19, 400 kHz
    // ===================================================
    let i2c = board::lpi2c(
        lpi2c1,
        pins.p19, // SCL
        pins.p18, // SDA
        board::Lpi2cClockSpeed::KHz400,
    );

    let i2c_bus = shared_bus::BusManagerSimple::new(i2c);
    let mut pca_i2c = i2c_bus.acquire_i2c();
    let adc_i2c = i2c_bus.acquire_i2c();

    const PCA_ADDR: u8 = 0x70;

    // ===================================================
    // I2C1 INITIALIZATION (bus kedua, terpisah dari i2c0 di atas)
    // SDA=17, SCL=16, 400 kHz
    // Dipakai untuk: SHT31 kedua + 2x sensor arus INA226
    // ===================================================
    let i2c1 = board::lpi2c(
        lpi2c3,
        pins.p16, // SCL
        pins.p17, // SDA
        board::Lpi2cClockSpeed::KHz400,
    );
    let i2c1_bus = shared_bus::BusManagerSimple::new(i2c1);

    // SHT31 (bus i2c1) - pin ADDR di-jumper ke VCC => alamat 0x45
    const SHT31_ADDR: u8 = 0x45;

    // INA226 (sensor arus, bus i2c1) - alamat dikonfirmasi lewat I2C scan
    // (lihat "I2C1 scan" di log), bukan cuma asumsi posisi jumper:
    //   - INA226 #1: alamat 0x41 (jumper A0 -> VCC, A1 -> GND) - mengukur
    //     arus & tegangan AKTUATOR, pakai shunt R005, arus ~4-5 A.
    //   - INA226 #2: alamat 0x40 (non-jumper / default) - mengukur arus &
    //     tegangan suplai SENSOR + MIKROKONTROLLER (bukan aktuator).
    //     Sebelumnya kode salah nembak ke 0x44, alamat yang di scan
    //     ternyata KOSONG (tidak ada device di sana), jadi jalur ini
    //     tidak pernah terbaca sama sekali sebelum diperbaiki.
    const INA226_ADDR: [u8; 2] = [0x41, 0x40];

    const INA226_REG_CONFIG: u8 = 0x00;
    const INA226_REG_SHUNT: u8 = 0x01;
    const INA226_REG_BUS: u8 = 0x02;
    const INA226_REG_POWER: u8 = 0x03;
    const INA226_REG_CURRENT: u8 = 0x04;
    const INA226_REG_CALIB: u8 = 0x05;

    // Shunt resistor sudah diganti dari 0.1 ohm (R100) menjadi 0.005 ohm (R005).
    // Arus terukur di lapangan sekitar 4-5 A, jadi CURRENT_LSB dinaikkan ke
    // 500 uA/bit (dari semula 100 uA/bit) supaya register current 16-bit
    // (maks +-32767 count) tidak overflow/clip sebelum sempat merepresentasikan
    // arus sebenarnya. Kalau shunt/rentang arus diganti lagi, hitung ulang:
    //   CAL = 0.00512 / (CURRENT_LSB * R_SHUNT)
    //       = 0.00512 / (0.0005 * 0.005) = 2048
    // Rentang maksimum sekarang: 32767 * 0.0005 A = ~16.38 A, persis mendekati
    // batas saturasi tegangan shunt R005 sendiri (+-81.92 mV / 0.005 ohm =
    // +-16.38 A), jadi keduanya sudah sinkron.
    const INA226_CURRENT_LSB: f32 = 0.0005; // 500 uA / bit

    // ---------------------------------------------------------------
    // KALIBRASI 2-TITIK (gain + offset), hasil regresi linear dari
    // Percobaan 2 (Idle/Purging/Inject PWM 20-100%) vs pembacaan avometer.
    // Data ini dari INA226 #1 (index 0, addr 0x41) = sensor arus AKTUATOR:
    //
    //   Idle          : sensor 2218 mA, avo 1713 mA
    //   Purging       : sensor 2283 mA, avo 1690 mA
    //   Inject PWM 20%: sensor 1391 mA, avo 1022 mA
    //   Inject PWM 40%: sensor 1360 mA, avo  994 mA
    //   Inject PWM 60%: sensor 1297 mA, avo  993 mA
    //   Inject PWM 80%: sensor 1317 mA, avo  997 mA
    //   Inject PWM100%: sensor 1341 mA, avo  990 mA
    //
    // Regresi: current_uncalibrated ≈ 1.2938 * arus_asli + 0.4498 (A)
    // Dibalik: arus_asli ≈ 0.773 * current_uncalibrated - 0.3477 (A)
    //
    // current_terkoreksi = current_uncalibrated * GAIN + OFFSET_A
    //
    // INA226 #2 (index 1, addr 0x40, sensor+MCU) memakai kalibrasi lama
    // dari Percobaan 1: offset-only, rata-rata (sensor - multimeter) =
    // 401.143 mA -> current_terkoreksi = current_uncalibrated - 0.401143,
    // ditulis di sini sebagai gain=1, offset=-0.401143.
    // ---------------------------------------------------------------
    const INA226_CURRENT_GAIN:     [f32; 2] = [0.773,    1.0];
    const INA226_CURRENT_OFFSET_A: [f32; 2] = [-0.3477, -0.401_143];

    const INA226_POWER_LSB: f32 = INA226_CURRENT_LSB * 25.0;
    const INA226_CAL_VALUE: u16 = 2048;
    const INA226_BUS_LSB: f32 = 0.00125; // 1.25 mV / bit (tetap, sesuai datasheet)
    const INA226_SHUNT_LSB: f32 = 0.0000025; // 2.5 uV / bit (tetap, sesuai datasheet)

    let mut ina226_i2c = i2c1_bus.acquire_i2c();

    // Scan sekali di startup untuk memastikan alamat fisik yang benar-benar ACK
    // di bus i2c1 ini (jangan cuma andalkan asumsi posisi jumper A0/A1).
    // PENTING: pakai write 1 byte dummy, bukan 0 byte - write kosong pada HAL
    // i2c ini tidak selalu sempat mendeteksi NACK fase alamat sebelum STOP
    // dikirim, sehingga hasilnya bisa ACK palsu di hampir semua alamat.
    usb_println!("I2C1 scan:");
    for addr in 0x03u8..=0x77u8 {
        if embedded_hal::blocking::i2c::Write::write(&mut ina226_i2c, addr, &[0x00]).is_ok() {
            usb_println!("  addr {:#04x}: ACK", addr);
        }
    }

    let mut ina226_ok = [false; 2];
    for i in 0..2 {
        let addr = INA226_ADDR[i];
        // Mode continuous shunt+bus, averaging 1x, conversion time 1.1ms (nilai default POR)
        ina226_write(&mut ina226_i2c, addr, INA226_REG_CONFIG, 0x4127);
        ina226_write(&mut ina226_i2c, addr, INA226_REG_CALIB, INA226_CAL_VALUE);
        my_usb_delay_ms!(2);
        ina226_ok[i] = ina226_read(&mut ina226_i2c, addr, INA226_REG_CALIB) == INA226_CAL_VALUE;
    }

    macro_rules! select_pca {
        ($channel:expr) => {
            let _ = pca_i2c.write(PCA_ADDR, &[1 << $channel]);
            my_usb_delay_ms!(5);
        };
    }

    let mut adc = Ads1x1x::new_ads1115(adc_i2c, SlaveAddr::default());
    let mut ads_ok = [false; 4];

    // ADS0
    select_pca!(0);
    if adc.set_full_scale_range(FullScaleRange::Within4_096V).is_ok() {
        ads_ok[0] = true;
    }

    // ADS1
    select_pca!(1);
    if adc.set_full_scale_range(FullScaleRange::Within4_096V).is_ok() {
        ads_ok[1] = true;
    }

    // ADS2
    select_pca!(2);
    if adc.set_full_scale_range(FullScaleRange::Within4_096V).is_ok() {
        ads_ok[2] = true;
    }

    // ADS3
    select_pca!(3);
    if adc.set_full_scale_range(FullScaleRange::Within4_096V).is_ok() {
        ads_ok[3] = true;
    }

    // ===================================================
    // SYSTEM STATUS
    // D31 = nyala selama sistem berjalan (heartbeat)
    // D32 = nyala kalau ada sensor ADS yang gagal init
    // ===================================================
    let _ = led_run.set_high();
    if ads_ok[0] && ads_ok[1] && ads_ok[2] && ads_ok[3] {
        let _ = led_error.set_low();
    } else {
        let _ = led_error.set_high();
    }

    usb_println!("SYSTEM READY");
    for i in 0..4 {
        usb_println!("ADS{}: {}", i, if ads_ok[i] { "OK" } else { "ERROR" });
    }
    for i in 0..2 {
        usb_println!(
            "INA226_{} (addr {:#x}): {}",
            i + 1,
            INA226_ADDR[i],
            if ina226_ok[i] { "OK" } else { "ERROR" }
        );
    }
    usb_println!("");

    let mut read_ads_raw = |ch: ChannelSelection| -> i16 {
        nb::block!(adc.read(ch)).unwrap_or(0)
    };

    loop {
        // NDIR CYCLE — one ON/OFF pair per loop iteration
        // Lamp ON → 125 ms → read ACT/REF → lamp OFF → 125 ms → read ACT/REF → process
            // Results stored in ndir_* vars, output with other sensors below.
            // ===================================================
            let mut _ndir_ratio:      f32 = 0.0;
            let mut ndir_ratio_f:    f32 = 0.0;
            let mut ndir_baseline:   f32 = 0.0;
            let mut _ndir_response:   f32 = 0.0;
            let mut ndir_response_f: f32 = 0.0;
            let mut _ndir_abs:        f32 = 0.0;
            let mut ndir_abs_f:      f32 = 0.0;
            let mut ndir_bl_ready:   bool = false;
            let mut ndir_valid:      bool = false;

            if ads_ok[3] {
                // Run 4 NDIR cycles (125ms ON, 125ms OFF) to make up ~1 second.
                // This keeps the lamp thermally oscillating continuously at 4 Hz,
                // while the rest of the sensors are reported at 1 Hz.
                for _ in 0..4 {
                    // ── ON phase ──────────────────────────────────────
                    let _ = ir_lamp.set_high();
                    my_usb_delay_ms!(NDIR_HALF_PERIOD_MS);

                    select_pca!(3);
                    let ndir_act_on_raw = read_ads_raw(ChannelSelection::SingleA1).max(0) as u16;
                    let ndir_ref_on_raw = read_ads_raw(ChannelSelection::SingleA2).max(0) as u16;

                    // Filter ON readings (Median5 → EMA)
                    ndir_act_cf.update(ndir_act_on_raw);
                    ndir_ref_cf.update(ndir_ref_on_raw);
                    let act_on_f = ndir_act_cf.filt_u16();
                    let ref_on_f = ndir_ref_cf.filt_u16();

                    // Store ON sample in state machine
                    ndir.capture_on(act_on_f, ref_on_f);

                    // ── OFF phase ─────────────────────────────────────
                    let _ = ir_lamp.set_low();
                    my_usb_delay_ms!(NDIR_HALF_PERIOD_MS);

                    select_pca!(3);
                    let ndir_act_off_raw = read_ads_raw(ChannelSelection::SingleA1).max(0) as u16;
                    let ndir_ref_off_raw = read_ads_raw(ChannelSelection::SingleA2).max(0) as u16;

                    // Filter OFF readings
                    ndir_act_cf.update(ndir_act_off_raw);
                    ndir_ref_cf.update(ndir_ref_off_raw);
                    let act_off_f = ndir_act_cf.filt_u16();
                    let ref_off_f = ndir_ref_cf.filt_u16();

                    // ── Process pair ──────────────────────────────────
                    if let Some(s) = ndir.capture_off_and_process(act_off_f, ref_off_f) {
                        let fp = ndir_proc_filt.update(
                            s.ratio, s.response_pct, s.baseline_ratio,
                        );
                        _ndir_ratio      = s.ratio;
                        ndir_ratio_f    = fp.ratio_filt;
                        ndir_baseline   = s.baseline_ratio;
                        _ndir_response   = s.response_pct;
                        ndir_response_f = fp.response_filt;
                        _ndir_abs        = fp.abs_raw;
                        ndir_abs_f      = fp.abs_filt;
                        ndir_bl_ready   = s.baseline_ready;
                        ndir_valid      = true;
                    }
                }
            } else {
                // If NDIR sensor is offline, simulate the 1-second delay so the loop cadence is intact
                my_usb_delay_ms!(1000);
            }

            // SENSOR VARIABLES (Raw i16)
            let mut mq2 = 0i16;
            let mut mq3 = 0i16;
            let mut mq4 = 0i16;
            let mut mq5 = 0i16;

            let mut mq6 = 0i16;
            let mut mq7 = 0i16;
            let mut mq8 = 0i16;
            let mut mq135 = 0i16;

            let mut tgs2600 = 0i16;
            let mut tgs2611 = 0i16;
            let mut tgs2610 = 0i16;
            let mut tgs822 = 0i16;

            let mut tgs813 = 0i16;
            let mut ir12emact = 0i16;
            let mut ir12emref = 0i16;
            let mut mq9 = 0i16;

            // ADS0
            if ads_ok[0] {
                select_pca!(0);
                mq2 = mox_filters[0].update_i16(read_ads_raw(ChannelSelection::SingleA0));
                mq3 = mox_filters[1].update_i16(read_ads_raw(ChannelSelection::SingleA1));
                mq4 = mox_filters[2].update_i16(read_ads_raw(ChannelSelection::SingleA2));
                mq5 = mox_filters[3].update_i16(read_ads_raw(ChannelSelection::SingleA3));
            }

            // ADS1
            if ads_ok[1] {
                select_pca!(1);
                mq6   = mox_filters[4].update_i16(read_ads_raw(ChannelSelection::SingleA0));
                mq7   = mox_filters[5].update_i16(read_ads_raw(ChannelSelection::SingleA1));
                mq8   = mox_filters[6].update_i16(read_ads_raw(ChannelSelection::SingleA2));
                mq135 = mox_filters[7].update_i16(read_ads_raw(ChannelSelection::SingleA3));
            }

            // ADS2
            if ads_ok[2] {
                select_pca!(2);
                tgs2600 = mox_filters[8].update_i16(read_ads_raw(ChannelSelection::SingleA0));
                tgs2611 = mox_filters[9].update_i16(read_ads_raw(ChannelSelection::SingleA1));
                tgs2610 = mox_filters[10].update_i16(read_ads_raw(ChannelSelection::SingleA2));
                tgs822  = mox_filters[11].update_i16(read_ads_raw(ChannelSelection::SingleA3));
            }

            // ADS3
            if ads_ok[3] {
                select_pca!(3);
                tgs813    = mox_filters[12].update_i16(read_ads_raw(ChannelSelection::SingleA0));
                ir12emact = read_ads_raw(ChannelSelection::SingleA1);
                ir12emref = read_ads_raw(ChannelSelection::SingleA2);
                mq9       = mox_filters[13].update_i16(read_ads_raw(ChannelSelection::SingleA3));
            }

            // FORMAT UNTUK WEB FRONTEND (ADC0 - ADC15)
            // Sesuai array GAS_SENSORS di Acquisition.jsx:
            usb_println!("ADC0 = {}", tgs2600);
            usb_println!("ADC1 = {}", tgs2611);
            usb_println!("ADC2 = {}", tgs2610);
            usb_println!("ADC3 = {}", tgs822);
            usb_println!("ADC4 = {}", tgs813);
            usb_println!("ADC5 = {}", mq2);
            usb_println!("ADC6 = {}", mq6);
            usb_println!("ADC7 = {}", mq8);
            usb_println!("ADC8 = {}", mq4);
            usb_println!("ADC9 = {}", mq3);
            usb_println!("ADC10 = {}", mq135);
            usb_println!("ADC11 = {}", mq9);
            usb_println!("ADC12 = {}", mq7);
            usb_println!("ADC13 = {}", mq5);
            usb_println!("ADC14 = {}", ir12emact);
            usb_println!("NDIR_VALID = {}", ndir_valid as u8);
            usb_println!("NDIR_BL_READY = {}", ndir_bl_ready as u8);
            usb_println!("NDIR_RATIO = {:.4}", ndir_ratio_f);
            usb_println!("NDIR_BASELINE = {:.4}", ndir_baseline);
            usb_println!("NDIR_RESPONSE = {:.2}", ndir_response_f);
            usb_println!("NDIR_ABSORBANCE = {:.4}", ndir_abs_f);
            usb_println!("ADC15 = {}", ir12emref); // Pastikan ini selalu dicetak paling akhir untuk trigger frontend

            // SHT30-D read
            let mut sht30_temp = 0.0;
            let mut sht30_hum = 0.0;
            let mut sht_i2c = i2c_bus.acquire_i2c();

            // Non-aktifkan semua channel PCA agar tidak ada interferensi di bus utama
            let _ = sht_i2c.write(0x70, &[0x00]);
            my_usb_delay_ms!(5);

            // Single shot, high repeatability, clock stretching enabled (same command as the
            // known-working Arduino reference: 0x2C06)
            match sht_i2c.write(0x44, &[0x2C, 0x06]) {
                Ok(_) => {
                    my_usb_delay_ms!(20);
                    let mut buf = [0u8; 6];
                    match embedded_hal::blocking::i2c::Read::read(&mut sht_i2c, 0x44, &mut buf) {
                        Ok(_) => {
                            let st = ((buf[0] as u32) << 8) | (buf[1] as u32);
                            let srh = ((buf[3] as u32) << 8) | (buf[4] as u32);
                            sht30_temp = -45.0 + 175.0 * (st as f32 / 65535.0);
                            sht30_hum = 100.0 * (srh as f32 / 65535.0);
                        },
                        Err(_) => usb_println!("SHT30: I2C Read failed at 0x44"),
                    }
                },
                Err(_) => usb_println!("SHT30: I2C Write failed at 0x44 (No ACK)"),
            }

            usb_println!("SHT30 Temp  = {:.2} C", sht30_temp);
            usb_println!("SHT30 Humi  = {:.2} %RH", sht30_hum);

            // KONTROL PELTIER - menggunakan Fuzzy PID
            if manual_pwr_mode {
                set_peltier_power(manual_pwr_val);
                peltier_mode = "MANUAL";
            } else if setpoint_c > 0.0 {
                let pwr = fuzzy_pid.compute(setpoint_c, sht30_temp, 1.0);
                set_peltier_power(pwr);
                if pwr > 0 {
                    peltier_mode = "COOL";
                } else if pwr < 0 {
                    peltier_mode = "HEAT";
                } else {
                    peltier_mode = "IDLE";
                }
            } else {
                set_peltier_power(0);
                peltier_mode = "OFF";
            }

            // SHT31 read (bus i2c1, pin 16/17)
            let mut sht31_temp = 0.0;
            let mut sht31_hum = 0.0;
            let mut sht31_i2c = i2c1_bus.acquire_i2c();

            match sht31_i2c.write(SHT31_ADDR, &[0x2C, 0x06]) {
                Ok(_) => {
                    my_usb_delay_ms!(20);
                    let mut buf = [0u8; 6];
                    match embedded_hal::blocking::i2c::Read::read(&mut sht31_i2c, SHT31_ADDR, &mut buf) {
                        Ok(_) => {
                            let st = ((buf[0] as u32) << 8) | (buf[1] as u32);
                            let srh = ((buf[3] as u32) << 8) | (buf[4] as u32);
                            sht31_temp = -45.0 + 175.0 * (st as f32 / 65535.0);
                            sht31_hum = 100.0 * (srh as f32 / 65535.0);
                        },
                        Err(_) => usb_println!("SHT31: I2C Read failed at {:#x}", SHT31_ADDR),
                    }
                },
                Err(_) => usb_println!("SHT31: I2C Write failed at {:#x} (No ACK)", SHT31_ADDR),
            }

            usb_println!("SHT31 Temp  = {:.2} C", sht31_temp);
            usb_println!("SHT31 Humi  = {:.2} %RH", sht31_hum);

            // SAFETY LOGIC
            if sht31_temp <= 15.0 || sht31_temp >= 40.0 || sht31_hum >= 85.0 {
                set_peltier_power(0);
                peltier_mode = "SAFETY_OFF";
                let _ = p15.set_low(); // Fan OFF
                usb_println!("SAFETY: Peltier & Fan D15 OFF (Out of bounds)");
            } else {
                let _ = p15.set_high(); // Fan ON
            }

            usb_println!("SETPOINT = {:.2} C", setpoint_c);
            usb_println!("PELTIER_MODE = {}", peltier_mode);

            // INA226 read (bus i2c1, 2x sensor arus)
            for i in 0..2 {
                if !ina226_ok[i] {
                    usb_println!(
                        "INA226_{} (addr {:#x}): ERROR (kalibrasi/I2C gagal, lewati)",
                        i + 1,
                        INA226_ADDR[i]
                    );
                    continue;
                }
                let addr = INA226_ADDR[i];
                let bus_raw = ina226_read(&mut ina226_i2c, addr, INA226_REG_BUS);
                let shunt_raw = ina226_read(&mut ina226_i2c, addr, INA226_REG_SHUNT) as i16;
                let current_raw = ina226_read(&mut ina226_i2c, addr, INA226_REG_CURRENT) as i16;
                let power_raw = ina226_read(&mut ina226_i2c, addr, INA226_REG_POWER);

                let bus_voltage = bus_raw as f32 * INA226_BUS_LSB;
                let shunt_voltage = shunt_raw as f32 * INA226_SHUNT_LSB;
                let current_uncalibrated = current_raw as f32 * INA226_CURRENT_LSB;
                // Kalibrasi 2-titik: gain dulu, baru offset ditambahkan
                // (lihat catatan di deklarasi INA226_CURRENT_GAIN/OFFSET_A di atas).
                let current = current_uncalibrated * INA226_CURRENT_GAIN[i] + INA226_CURRENT_OFFSET_A[i];
                let power = power_raw as f32 * INA226_POWER_LSB;

                usb_println!(
                    "INA226_{} Vbus = {:.3} V, Vshunt = {:.5} V, I = {:.4} A, P = {:.4} W",
                    i + 1,
                    bus_voltage,
                    shunt_voltage,
                    current,
                    power
                );
            }
    }
}
