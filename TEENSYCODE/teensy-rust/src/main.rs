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
        let mut chunk = [0u8; 32];
        let n = self.serial.read(&mut chunk).unwrap_or(0);
        let mut result = None;
        for &b in &chunk[..n] {
            if b == b'\n' || b == b'\r' {
                if *rx_len > 0 {
                    if let Ok(line) = core::str::from_utf8(&rx_buf[..*rx_len]) {
                        let line = line.trim();
                        if let Some(rest) = line.strip_prefix("SETPOINT=") {
                            if let Ok(val) = rest.trim().parse::<f32>() {
                                result = Some(HostCmd::Setpoint(val));
                            }
                        } else if let Some(rest) = line.strip_prefix("PHASE=") {
                            match rest.trim() {
                                "IDLE" => result = Some(HostCmd::Phase("IDLE")),
                                "INJECT" => result = Some(HostCmd::Phase("INJECT")),
                                "PURGE" => result = Some(HostCmd::Phase("PURGE")),
                                "OFF" => result = Some(HostCmd::Phase("OFF")),
                                _ => {}
                            }
                        } else if let Some(rest) = line.strip_prefix("PWM=") {
                            if let Ok(val) = rest.trim().parse::<u16>() {
                                result = Some(HostCmd::Pump1Pwm(val));
                            }
                        }
                    }
                    *rx_len = 0;
                }
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

    // Tunggu device selesai enumerasi (sepenuhnya diservis oleh interrupt,
    // bukan polling manual di sini) - timeout kasar supaya board tetap
    // bisa boot standalone kalau tidak ada host yang connect.
    for _ in 0..3_000_000u32 {
        if with_usb_comm(|comm| comm.configured).unwrap_or(false) {
            break;
        }
    }

    delay.block_ms(500);

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


    // ===================================================
    // PUMP 1 - motor driver BTS7960
    // ===================================================
    let mut pump1_l_en = gpio4.output(pins.p5);
    let _ = pump1_l_en.set_high(); // Enable bridge L
    
    let mut pump1_r_en = gpio4.output(pins.p4);
    let _ = pump1_r_en.set_high(); // Enable bridge R
    
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
        pump1_lpwm_out.set_output_enable(&mut pwm4, false);
        if permille == 0 {
            pump1_rpwm_out.set_output_enable(&mut pwm4, false);
        } else {
            pump1_rpwm_out.set_turn_off(&pwm4_sm2, pwm_permille_to_turn_off(permille));
            pwm4_sm2.set_load_ok(&mut pwm4);
            pump1_rpwm_out.set_output_enable(&mut pwm4, true);
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
    let _ = peltier_l_en.set_high();

    let mut peltier_r_en = gpio2.output(pins.p8);
    let _ = peltier_r_en.set_high();

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
    const PELTIER_HYSTERESIS_C: f32 = 0.5;
    const PELTIER_COOL_POWER_PERCENT: i16 = 80;
    const PELTIER_HEAT_POWER_PERCENT: i16 = 80;
    let mut setpoint_c: f32 = 25.0;
    let mut peltier_mode: &str = "OFF";

    let mut phase_mode: &str = "OFF";
    let mut pump1_pwm_val: u16 = 500; // default 50%

    let mut setpoint_rx_buf = [0u8; 48];
    let mut setpoint_rx_len: usize = 0;

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

    // INA226 (sensor arus, bus i2c1) - alamat ditentukan jumper A0/A1 pada modul:
    //   - INA226 #1: jumper A0 -> VCC, A1 -> GND  => alamat 0x41
    //   - INA226 #2: jumper A1 -> VCC, A0 -> GND  => alamat 0x44
    const INA226_ADDR: [u8; 2] = [0x41, 0x44];

    const INA226_REG_CONFIG: u8 = 0x00;
    const INA226_REG_SHUNT: u8 = 0x01;
    const INA226_REG_BUS: u8 = 0x02;
    const INA226_REG_POWER: u8 = 0x03;
    const INA226_REG_CURRENT: u8 = 0x04;
    const INA226_REG_CALIB: u8 = 0x05;

    // Asumsi shunt resistor 0.1 ohm, arus maksimum ~3.2 A (umum pada modul breakout
    // INA226 di pasaran). Jika shunt resistor pada modul Anda berbeda, sesuaikan
    // INA226_CURRENT_LSB dan INA226_CAL_VALUE:
    //   CAL = 0.00512 / (CURRENT_LSB * R_SHUNT)
    const INA226_CURRENT_LSB: f32 = 0.0001; // 100 uA / bit
    const INA226_POWER_LSB: f32 = INA226_CURRENT_LSB * 25.0;
    const INA226_CAL_VALUE: u16 = 512;
    const INA226_BUS_LSB: f32 = 0.00125; // 1.25 mV / bit (tetap, sesuai datasheet)
    const INA226_SHUNT_LSB: f32 = 0.0000025; // 2.5 uV / bit (tetap, sesuai datasheet)

    let mut ina226_i2c = i2c1_bus.acquire_i2c();
    let mut ina226_ok = [false; 2];
    for i in 0..2 {
        let addr = INA226_ADDR[i];
        // Mode continuous shunt+bus, averaging 1x, conversion time 1.1ms (nilai default POR)
        ina226_write(&mut ina226_i2c, addr, INA226_REG_CONFIG, 0x4127);
        ina226_write(&mut ina226_i2c, addr, INA226_REG_CALIB, INA226_CAL_VALUE);
        delay.block_ms(2);
        ina226_ok[i] = ina226_read(&mut ina226_i2c, addr, INA226_REG_CALIB) == INA226_CAL_VALUE;
    }

    let mut select_pca = |channel: u8, delay: &mut Blocking<_, { board::PERCLK_FREQUENCY }>| {
        let _ = pca_i2c.write(PCA_ADDR, &[1 << channel]);
        delay.block_ms(5);
    };

    let mut adc = Ads1x1x::new_ads1115(adc_i2c, SlaveAddr::default());
    let mut ads_ok = [false; 4];

    // ADS0
    select_pca(0, &mut delay);
    if adc.set_full_scale_range(FullScaleRange::Within4_096V).is_ok() {
        ads_ok[0] = true;
    }

    // ADS1
    select_pca(1, &mut delay);
    if adc.set_full_scale_range(FullScaleRange::Within4_096V).is_ok() {
        ads_ok[1] = true;
    }

    // ADS2
    select_pca(2, &mut delay);
    if adc.set_full_scale_range(FullScaleRange::Within4_096V).is_ok() {
        ads_ok[2] = true;
    }

    // ADS3
    select_pca(3, &mut delay);
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
        for _gpio_idx in 0..6 {
            if let Some(cmd) =
                with_usb_comm(|comm| comm.poll_cmd(&mut setpoint_rx_buf, &mut setpoint_rx_len))
                    .flatten()
            {
                match cmd {
                    HostCmd::Setpoint(val) => {
                        setpoint_c = val;
                        usb_println!("ACK SETPOINT = {:.2} C", setpoint_c);
                    },
                    HostCmd::Phase(p) => {
                        phase_mode = p;
                        usb_println!("ACK PHASE = {}", p);
                    },
                    HostCmd::Pump1Pwm(val) => {
                        let val_permille = (val.min(100) as u16) * 10;
                        pump1_pwm_val = val_permille;
                        usb_println!("ACK PWM = {}%", val.min(100));
                        if phase_mode == "INJECT" {
                            set_pump1_pwm(pump1_pwm_val);
                        }
                    }
                }
            }
            
            // Handle Phases
            match phase_mode {
                "IDLE" => {
                    let _ = p14.set_high(); // Pump 2
                    let _ = p38.set_high(); // SV1
                    let _ = p39.set_high(); // SV2
                    
                    let _ = p40.set_low(); // SV3
                    let _ = p41.set_low(); // Pump 3
                    set_pump1_pwm(0);
                },
                "INJECT" => {
                    set_pump1_pwm(pump1_pwm_val); // Pump 1
                    let _ = p40.set_high(); // SV3
                    
                    let _ = p14.set_low(); // Pump 2
                    let _ = p41.set_low(); // Pump 3
                    let _ = p38.set_low(); // SV1
                    let _ = p39.set_low(); // SV2
                },
                "PURGE" => {
                    let _ = p41.set_high(); // Pump 3
                    let _ = p39.set_high(); // SV2
                    let _ = p38.set_high(); // SV1
                    
                    let _ = p40.set_low(); // SV3
                    let _ = p14.set_low(); // Pump 2
                    set_pump1_pwm(0);
                },
                _ => { // OFF
                    let _ = p14.set_low();
                    let _ = p38.set_low();
                    let _ = p39.set_low();
                    let _ = p40.set_low();
                    let _ = p41.set_low();
                    set_pump1_pwm(0);
                }
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
                select_pca(0, &mut delay);
                mq2 = read_ads_raw(ChannelSelection::SingleA0);
                mq3 = read_ads_raw(ChannelSelection::SingleA1);
                mq4 = read_ads_raw(ChannelSelection::SingleA2);
                mq5 = read_ads_raw(ChannelSelection::SingleA3);
            }

            // ADS1
            if ads_ok[1] {
                select_pca(1, &mut delay);
                mq6 = read_ads_raw(ChannelSelection::SingleA0);
                mq7 = read_ads_raw(ChannelSelection::SingleA1);
                mq8 = read_ads_raw(ChannelSelection::SingleA2);
                mq135 = read_ads_raw(ChannelSelection::SingleA3);
            }

            // ADS2
            if ads_ok[2] {
                select_pca(2, &mut delay);
                tgs2600 = read_ads_raw(ChannelSelection::SingleA0);
                tgs2611 = read_ads_raw(ChannelSelection::SingleA1);
                tgs2610 = read_ads_raw(ChannelSelection::SingleA2);
                tgs822 = read_ads_raw(ChannelSelection::SingleA3);
            }

            // ADS3
            if ads_ok[3] {
                select_pca(3, &mut delay);
                tgs813 = read_ads_raw(ChannelSelection::SingleA0);
                ir12emact = read_ads_raw(ChannelSelection::SingleA1);
                ir12emref = read_ads_raw(ChannelSelection::SingleA2);
                mq9 = read_ads_raw(ChannelSelection::SingleA3);
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
            usb_println!("ADC15 = {}", ir12emref); // Pastikan ini selalu dicetak paling akhir untuk trigger frontend

            // SHT30-D read
            let mut sht30_temp = 0.0;
            let mut sht30_hum = 0.0;
            let mut sht_i2c = i2c_bus.acquire_i2c();

            // Non-aktifkan semua channel PCA agar tidak ada interferensi di bus utama
            let _ = sht_i2c.write(0x70, &[0x00]);
            delay.block_ms(5);

            // Single shot, high repeatability, clock stretching enabled (same command as the
            // known-working Arduino reference: 0x2C06)
            match sht_i2c.write(0x44, &[0x2C, 0x06]) {
                Ok(_) => {
                    delay.block_ms(20);
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

            // KONTROL PELTIER - berbasis suhu chamber (SHT30 di atas), lihat
            // penjelasan bang-bang + hysteresis di dekat inisialisasi Peltier.
            if sht30_temp > setpoint_c + PELTIER_HYSTERESIS_C {
                set_peltier_power(PELTIER_COOL_POWER_PERCENT);
                peltier_mode = "COOL";
            } else if sht30_temp < setpoint_c - PELTIER_HYSTERESIS_C {
                set_peltier_power(-PELTIER_HEAT_POWER_PERCENT);
                peltier_mode = "HEAT";
            }
            usb_println!("SETPOINT = {:.2} C", setpoint_c);
            usb_println!("PELTIER_MODE = {}", peltier_mode);

            // SHT31 read (bus i2c1, pin 16/17)
            let mut sht31_temp = 0.0;
            let mut sht31_hum = 0.0;
            let mut sht31_i2c = i2c1_bus.acquire_i2c();

            match sht31_i2c.write(SHT31_ADDR, &[0x2C, 0x06]) {
                Ok(_) => {
                    delay.block_ms(20);
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

            // INA226 read (bus i2c1, 2x sensor arus)
            for i in 0..2 {
                if !ina226_ok[i] {
                    continue;
                }
                let addr = INA226_ADDR[i];
                let bus_raw = ina226_read(&mut ina226_i2c, addr, INA226_REG_BUS);
                let shunt_raw = ina226_read(&mut ina226_i2c, addr, INA226_REG_SHUNT) as i16;
                let current_raw = ina226_read(&mut ina226_i2c, addr, INA226_REG_CURRENT) as i16;
                let power_raw = ina226_read(&mut ina226_i2c, addr, INA226_REG_POWER);

                let bus_voltage = bus_raw as f32 * INA226_BUS_LSB;
                let shunt_voltage = shunt_raw as f32 * INA226_SHUNT_LSB;
                let current = current_raw as f32 * INA226_CURRENT_LSB;
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

            delay.block_ms(1000);
        }
    }
}
