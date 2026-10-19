//! Inferensi HW via FPGA Kria KV260 tanpa PYNQ.
//!
//! Register map dari HWH (base 0xB0000000):
//!   0x00  CTRL        — bit0=AP_START, bit1=AP_DONE, bit2=AP_IDLE
//!   0x10  x_raw_1     — ptr input [31:0]
//!   0x14  x_raw_2     — ptr input [63:32]
//!   0x1C  logits_1    — ptr output [31:0]
//!   0x20  logits_2    — ptr output [63:32]
//!   0x28  pred_r      — predicted class index (read-only)
//!
//! Input buffer : 37 × f32 = 148 byte
//! Output buffer: 5  × f32 = 20  byte
//!
//! Memori fisik dialokasikan via /dev/udmabuf0 atau fixed DDR.
//! Load bitstream via shell: fpgautil -b snn.bit -o snn.hwh

use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::time::{Duration, Instant};

const SNN_BASE:     u64 = 0xB000_0000;
const SNN_RANGE:    usize = 0x1_0000;
const REG_CTRL:     usize = 0x00;
const REG_X_LO:     usize = 0x10;
const REG_X_HI:     usize = 0x14;
const REG_LOGITS_LO: usize = 0x1C;
const REG_LOGITS_HI: usize = 0x20;
const AP_START:     u32 = 1;
const AP_DONE:      u32 = 1 << 1;
const AP_IDLE:      u32 = 1 << 2;

// Alamat fisik DDR untuk buffer DMA (sesuai reserved-memory atau udmabuf)
const X_PHYS:    u64 = 0x7000_0000;  // 148 byte input
const LOGIT_PHYS: u64 = 0x7000_1000; // 20  byte output

/// Muat bitstream ke FPGA via fpgautil.
#[allow(dead_code)]
pub fn load_bitstream(bit: &Path, hwh: &Path) -> io::Result<()> {
    let status = std::process::Command::new("fpgautil")
        .args([
            "-b", bit.to_str().unwrap_or(""),
            "-o", hwh.to_str().unwrap_or(""),
        ])
        .status()?;
    if !status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "fpgautil gagal memuat bitstream"));
    }
    Ok(())
}

/// MMap wrapper atas /dev/mem.
struct MemMap {
    ptr:  *mut u8,
    size: usize,
}

impl MemMap {
    fn open(phys: u64, size: usize) -> io::Result<Self> {
        let page = 4096usize;
        let offset   = (phys as usize) % page;
        let aligned  = phys - offset as u64;
        let map_size = size + offset;
        let fd = OpenOptions::new().read(true).write(true)
            .custom_flags(libc_o_sync())
            .open("/dev/mem")?;
        let ptr = unsafe {
            libc_mmap(map_size, fd.as_raw_fd(), aligned as i64)
        };
        if ptr.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(MemMap { ptr: unsafe { ptr.add(offset) }, size })
    }

    #[inline] unsafe fn read32(&self, off: usize) -> u32 {
        (self.ptr.add(off) as *const u32).read_volatile()
    }
    #[inline] unsafe fn write32(&self, off: usize, v: u32) {
        (self.ptr.add(off) as *mut u32).write_volatile(v);
    }
    #[inline] unsafe fn write_f32_slice(&self, off: usize, data: &[f32]) {
        let dst = self.ptr.add(off) as *mut f32;
        for (i, &v) in data.iter().enumerate() { dst.add(i).write_volatile(v); }
    }
    #[inline] unsafe fn read_f32_slice(&self, off: usize, dst: &mut [f32]) {
        let src = self.ptr.add(off) as *const f32;
        for (i, d) in dst.iter_mut().enumerate() { *d = src.add(i).read_volatile(); }
    }
}

impl Drop for MemMap {
    fn drop(&mut self) { unsafe { libc_munmap(self.ptr, self.size); } }
}

// Thin libc wrappers (tidak import crate libc agar dependensi minimal)
fn libc_o_sync() -> i32 { 0x101000 }  // O_RDWR | O_SYNC on aarch64 Linux
extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn munmap(addr: *mut u8, len: usize) -> i32;
}
unsafe fn libc_mmap(size: usize, fd: i32, offset: i64) -> *mut u8 {
    mmap(std::ptr::null_mut(), size, 0x3 /*PROT_RW*/, 0x1 /*MAP_SHARED*/, fd, offset)
}
unsafe fn libc_munmap(ptr: *mut u8, size: usize) { munmap(ptr, size); }

/// Jalankan satu inferensi FPGA HW.
/// Input: 37 float32 fitur (sudah dinormalisasi oleh akselerator / baked in).
/// Output: 5 float32 logit.
pub fn infer_hw(features_normalized: &[f32; 37], timeout: Duration)
    -> Result<[f32; 5], Box<dyn std::error::Error>>
{
    // Map register IP
    let regs = MemMap::open(SNN_BASE, SNN_RANGE)?;
    // Map buffer input (x) dan output (logits) di DDR
    let x_buf   = MemMap::open(X_PHYS,    37 * 4)?;
    let out_buf  = MemMap::open(LOGIT_PHYS, 5 * 4)?;

    unsafe {
        // Tulis input ke DDR
        x_buf.write_f32_slice(0, features_normalized);

        // Set pointer register
        regs.write32(REG_X_LO,     (X_PHYS & 0xFFFF_FFFF) as u32);
        regs.write32(REG_X_HI,     (X_PHYS >> 32) as u32);
        regs.write32(REG_LOGITS_LO, (LOGIT_PHYS & 0xFFFF_FFFF) as u32);
        regs.write32(REG_LOGITS_HI, (LOGIT_PHYS >> 32) as u32);

        // Tunggu idle
        let t0 = Instant::now();
        while (regs.read32(REG_CTRL) & AP_IDLE) == 0 {
            if t0.elapsed() > timeout {
                return Err("FPGA tidak idle (timeout)".into());
            }
            std::hint::spin_loop();
        }

        // Trigger
        regs.write32(REG_CTRL, AP_START);

        // Tunggu selesai (AP_DONE atau kembali ke AP_IDLE)
        let t0 = Instant::now();
        loop {
            let ctrl = regs.read32(REG_CTRL);
            if (ctrl & AP_DONE) != 0 || (ctrl & AP_IDLE) != 0 { break; }
            if t0.elapsed() > timeout {
                return Err("FPGA tidak selesai (timeout)".into());
            }
            std::hint::spin_loop();
        }

        // Baca hasil
        let mut logits = [0.0f32; 5];
        out_buf.read_f32_slice(0, &mut logits);
        Ok(logits)
    }
}
