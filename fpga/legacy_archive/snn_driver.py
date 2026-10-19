#!/usr/bin/env python3
"""
SNN FPGA Driver for Kria KV260

Loads params/input data into CMA (contiguous memory) buffers,
programs the SNN HLS IP registers, triggers inference, and reads results.

HLS IP Register Map (from HWH):
  Offset 0x00: CTRL        — AP_START(bit0), AP_DONE(bit1), AP_IDLE(bit2), AP_READY(bit3)
  Offset 0x10: x_1         — input pointer [31:0]
  Offset 0x14: x_2         — input pointer [63:32]
  Offset 0x1C: params_1    — params pointer [31:0]
  Offset 0x20: params_2    — params pointer [63:32]
  Offset 0x28: out_r_1     — output pointer [31:0]
  Offset 0x2C: out_r_2     — output pointer [63:32]

AXI Master interfaces:
  m_axi_gmem_x  — reads input (x)
  m_axi_gmem_p  — reads params
  m_axi_gmem_o  — writes output (out_r)

All three go through SmartConnect → PS S_AXI_HP0_FPD → DDR.

Usage:
  sudo python3 fpga/snn_driver.py                    # run test inference
  sudo python3 fpga/snn_driver.py --input data.bin    # custom input
"""
import mmap
import os
import struct
import sys
import time
import argparse
import numpy as np

# ============================================================
# Constants from HWH
# ============================================================
SNN_BASE_ADDR = 0xB0000000
SNN_ADDR_RANGE = 0x10000

# Register offsets
REG_CTRL = 0x00
REG_GIER = 0x04
REG_IP_IER = 0x08
REG_IP_ISR = 0x0C
REG_X_LO = 0x10
REG_X_HI = 0x14
REG_PARAMS_LO = 0x1C
REG_PARAMS_HI = 0x20
REG_OUT_LO = 0x28
REG_OUT_HI = 0x2C

# Control register bits
AP_START = 0x01
AP_DONE = 0x02
AP_IDLE = 0x04
AP_READY = 0x08

# CMA allocator via /dev/cma or fallback to udmabuf/mmap'd /dev/mem
CMA_DEVICE = "/dev/udmabuf0"
MEM_DEVICE = "/dev/mem"

PAGE_SIZE = 4096


class MmapRegister:
    """Memory-mapped register access via /dev/mem."""

    def __init__(self, base_addr: int, size: int):
        self.fd = os.open(MEM_DEVICE, os.O_RDWR | os.O_SYNC)
        # Align to page boundary
        self.base_addr = base_addr
        page_offset = base_addr % PAGE_SIZE
        aligned_addr = base_addr - page_offset
        map_size = size + page_offset
        self.mm = mmap.mmap(
            self.fd, map_size,
            mmap.MAP_SHARED,
            mmap.PROT_READ | mmap.PROT_WRITE,
            offset=aligned_addr
        )
        self.offset = page_offset

    def read32(self, reg_offset: int) -> int:
        pos = self.offset + reg_offset
        self.mm.seek(pos)
        return struct.unpack("<I", self.mm.read(4))[0]

    def write32(self, reg_offset: int, value: int):
        pos = self.offset + reg_offset
        self.mm.seek(pos)
        self.mm.write(struct.pack("<I", value & 0xFFFFFFFF))

    def close(self):
        self.mm.close()
        os.close(self.fd)


class CMABuffer:
    """
    Physically contiguous buffer for DMA.

    Tries /dev/udmabuf0 first, falls back to allocating from a known
    physical address range via /dev/mem.
    """

    def __init__(self, size: int, phys_addr: int = None):
        self.size = size
        self.phys_addr = phys_addr if phys_addr else self._alloc_phys(size)
        self.fd = os.open(MEM_DEVICE, os.O_RDWR | os.O_SYNC)
        page_offset = self.phys_addr % PAGE_SIZE
        aligned = self.phys_addr - page_offset
        map_size = size + page_offset
        self.mm = mmap.mmap(
            self.fd, map_size,
            mmap.MAP_SHARED,
            mmap.PROT_READ | mmap.PROT_WRITE,
            offset=aligned
        )
        self.offset = page_offset

    @staticmethod
    def _alloc_phys(size: int) -> int:
        """
        Use a high DDR address region unlikely to conflict with Linux.
        In PetaLinux on KV260 with 4GB DDR, addresses above 0x70000000
        in the low DDR range (0-0x80000000) are usually safe for DMA
        buffers of a few MB. Alternatively use reserved-memory in DTS.
        """
        # We use a fixed high address. For production, use CMA or
        # reserved-memory in the device tree.
        return 0x70000000

    def write(self, data: bytes, offset: int = 0):
        self.mm.seek(self.offset + offset)
        self.mm.write(data)

    def read(self, length: int, offset: int = 0) -> bytes:
        self.mm.seek(self.offset + offset)
        return self.mm.read(length)

    def close(self):
        self.mm.close()
        os.close(self.fd)


def wait_for_idle(regs: MmapRegister, timeout: float = 5.0):
    """Wait until the IP is idle."""
    t0 = time.monotonic()
    while True:
        ctrl = regs.read32(REG_CTRL)
        if ctrl & AP_IDLE:
            return
        if time.monotonic() - t0 > timeout:
            raise TimeoutError(f"SNN IP not idle after {timeout}s (CTRL=0x{ctrl:08x})")
        time.sleep(0.0001)


def wait_for_done(regs: MmapRegister, timeout: float = 10.0):
    """Wait until the IP signals AP_DONE or returns to AP_IDLE.
    
    Some HLS IPs auto-clear AP_DONE and return directly to IDLE.
    After writing AP_START, the IP goes busy (IDLE clears). So if
    IDLE reappears, the computation is complete.
    """
    t0 = time.monotonic()
    while True:
        ctrl = regs.read32(REG_CTRL)
        if ctrl & AP_DONE:
            return
        if ctrl & AP_IDLE:
            # IP returned to idle → computation completed (AP_DONE was auto-cleared)
            return
        if time.monotonic() - t0 > timeout:
            raise TimeoutError(f"SNN IP not done after {timeout}s (CTRL=0x{ctrl:08x})")
        time.sleep(0.0001)


def run_inference(
    input_data: bytes,
    params_data: bytes,
    output_size: int,
    verbose: bool = True
) -> bytes:
    """
    Run one SNN inference on the FPGA.

    Args:
        input_data:  raw bytes of input features (float32)
        params_data: raw bytes of network parameters (float32)
        output_size: expected output size in bytes
        verbose:     print debug info

    Returns:
        Raw output bytes
    """
    # Memory layout in DDR (physical addresses):
    #   params_buf:  0x70000000  (largest, ~3.8 MB)
    #   input_buf:   params + aligned(params_size)
    #   output_buf:  input + aligned(input_size)
    ALIGN = 4096
    params_size_aligned = ((len(params_data) + ALIGN - 1) // ALIGN) * ALIGN
    input_size_aligned = ((len(input_data) + ALIGN - 1) // ALIGN) * ALIGN
    output_size_aligned = ((output_size + ALIGN - 1) // ALIGN) * ALIGN

    params_phys = 0x70000000
    input_phys = params_phys + params_size_aligned
    output_phys = input_phys + input_size_aligned

    total_needed = params_size_aligned + input_size_aligned + output_size_aligned
    if verbose:
        print(f"Memory layout:")
        print(f"  params:  0x{params_phys:08x} ({len(params_data)} bytes)")
        print(f"  input:   0x{input_phys:08x}  ({len(input_data)} bytes)")
        print(f"  output:  0x{output_phys:08x}  ({output_size} bytes)")
        print(f"  total:   {total_needed / 1024:.1f} KB")

    # Allocate buffers
    params_buf = CMABuffer(len(params_data), params_phys)
    input_buf = CMABuffer(len(input_data), input_phys)
    output_buf = CMABuffer(output_size, output_phys)

    try:
        # Write data to DMA buffers
        if verbose:
            print("\nWriting params to DDR ...")
        params_buf.write(params_data)

        if verbose:
            print("Writing input to DDR ...")
        input_buf.write(input_data)

        # Zero output buffer
        output_buf.write(b'\x00' * output_size)

        # Open register space
        regs = MmapRegister(SNN_BASE_ADDR, SNN_ADDR_RANGE)

        try:
            # Wait for IP to be idle
            if verbose:
                print("Waiting for IP idle ...")
            wait_for_idle(regs)

            ctrl = regs.read32(REG_CTRL)
            if verbose:
                print(f"  CTRL = 0x{ctrl:08x} (idle={bool(ctrl & AP_IDLE)})")

            # Set pointer registers (64-bit physical addresses, split lo/hi)
            regs.write32(REG_X_LO, input_phys & 0xFFFFFFFF)
            regs.write32(REG_X_HI, (input_phys >> 32) & 0xFFFFFFFF)

            regs.write32(REG_PARAMS_LO, params_phys & 0xFFFFFFFF)
            regs.write32(REG_PARAMS_HI, (params_phys >> 32) & 0xFFFFFFFF)

            regs.write32(REG_OUT_LO, output_phys & 0xFFFFFFFF)
            regs.write32(REG_OUT_HI, (output_phys >> 32) & 0xFFFFFFFF)

            if verbose:
                print("\nStarting inference ...")

            t_start = time.monotonic()

            # Trigger AP_START
            regs.write32(REG_CTRL, AP_START)

            # Wait for completion
            wait_for_done(regs, timeout=30.0)

            t_elapsed = time.monotonic() - t_start
            if verbose:
                print(f"  Inference done in {t_elapsed * 1000:.2f} ms")

            # Read results
            result = output_buf.read(output_size)
            return result

        finally:
            regs.close()
    finally:
        params_buf.close()
        input_buf.close()
        output_buf.close()


def main():
    parser = argparse.ArgumentParser(description="SNN FPGA Inference Driver")
    parser.add_argument("--input", default="fpga/bitstream/test_input.bin",
                        help="Input binary file (float32)")
    parser.add_argument("--params", default="fpga/bitstream/params.bin",
                        help="Parameters binary file")
    parser.add_argument("--expected", default="fpga/bitstream/test_expected.bin",
                        help="Expected output for verification (optional)")
    parser.add_argument("--output-size", type=int, default=None,
                        help="Output size in bytes (auto-detected from expected)")
    parser.add_argument("-q", "--quiet", action="store_true")
    args = parser.parse_args()

    verbose = not args.quiet

    # Load input data
    with open(args.input, "rb") as f:
        input_data = f.read()
    if verbose:
        n_floats = len(input_data) // 4
        print(f"Input:  {args.input} ({len(input_data)} bytes, {n_floats} float32)")

    # Load params
    with open(args.params, "rb") as f:
        params_data = f.read()
    if verbose:
        print(f"Params: {args.params} ({len(params_data)} bytes, {len(params_data)/1024:.1f} KB)")

    # Determine output size
    output_size = args.output_size
    expected = None
    if output_size is None:
        if os.path.exists(args.expected):
            with open(args.expected, "rb") as f:
                expected = f.read()
            output_size = len(expected)
        else:
            # Default: assume 5 float32 outputs (5 DGA fault classes)
            output_size = 20

    if verbose:
        print(f"Output: {output_size} bytes ({output_size // 4} float32)")
        print("")

    # Run inference
    result = run_inference(input_data, params_data, output_size, verbose)

    # Parse and display results
    n_out = output_size // 4
    outputs = struct.unpack(f"<{n_out}f", result)

    print(f"\n{'='*50}")
    print(f"SNN Output ({n_out} values):")
    for i, v in enumerate(outputs):
        print(f"  [{i}] = {v:.6f}")

    # Compare with expected if available
    if expected is not None:
        expected_vals = struct.unpack(f"<{n_out}f", expected)
        print(f"\nExpected:")
        for i, v in enumerate(expected_vals):
            print(f"  [{i}] = {v:.6f}")

        # Check closeness
        max_err = max(abs(outputs[i] - expected_vals[i]) for i in range(n_out))
        print(f"\nMax absolute error: {max_err:.6f}")
        if max_err < 0.01:
            print("✅ PASS — outputs match expected values!")
        else:
            print("⚠️  MISMATCH — outputs differ from expected values")

    # DGA fault classification interpretation
    DGA_CLASSES = [
        "Normal",
        "Thermal Fault",
        "Electrical Fault (Low Energy)",
        "Electrical Fault (High Energy)",
        "Partial Discharge",
    ]
    if n_out == len(DGA_CLASSES):
        print(f"\n{'='*50}")
        print("DGA Classification Results:")
        for i, (cls, val) in enumerate(zip(DGA_CLASSES, outputs)):
            bar = "█" * int(val * 40)
            print(f"  {cls:35s} {val:6.3f} {bar}")
        predicted = max(range(n_out), key=lambda i: outputs[i])
        print(f"\n  → Predicted: {DGA_CLASSES[predicted]} ({outputs[predicted]:.3f})")


if __name__ == "__main__":
    main()
