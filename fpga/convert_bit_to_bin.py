#!/usr/bin/env python3
"""
Convert Xilinx .bit to .bin for the Linux FPGA Manager.

The FPGA manager on ZynqMP expects a raw bitstream (.bin) with
32-bit words byte-swapped relative to the .bit format.
"""
import struct
import sys
import os


def bit_to_bin(bit_path: str, bin_path: str):
    with open(bit_path, "rb") as f:
        data = f.read()

    # --- Parse Xilinx .bit header ---
    # Field 1: 2-byte length + dummy bytes
    idx = 0
    hdr_len = struct.unpack(">H", data[idx:idx + 2])[0]
    idx += 2 + hdr_len

    # Field 2: 2-byte key length
    key_len = struct.unpack(">H", data[idx:idx + 2])[0]
    idx += 2

    # Skip tagged fields: 'a' (design name), 'b' (part), 'c' (date), 'd' (time)
    for tag in (b"a", b"b", b"c", b"d"):
        assert data[idx:idx + 1] == tag, f"Expected tag {tag!r}, got {data[idx:idx+1]!r}"
        idx += 1
        field_len = struct.unpack(">H", data[idx:idx + 2])[0]
        idx += 2 + field_len

    # Field 'e': bitstream payload
    assert data[idx:idx + 1] == b"e", f"Expected tag 'e', got {data[idx:idx+1]!r}"
    idx += 1
    payload_len = struct.unpack(">I", data[idx:idx + 4])[0]
    idx += 4
    payload = data[idx:idx + payload_len]

    # Byte-swap 32-bit words (big-endian -> little-endian for FPGA manager)
    assert len(payload) % 4 == 0, "Payload length not multiple of 4"
    swapped = bytearray(len(payload))
    for i in range(0, len(payload), 4):
        swapped[i] = payload[i + 3]
        swapped[i + 1] = payload[i + 2]
        swapped[i + 2] = payload[i + 1]
        swapped[i + 3] = payload[i]

    with open(bin_path, "wb") as f:
        f.write(swapped)

    print(f"Converted: {bit_path} -> {bin_path}")
    print(f"  Payload size: {len(swapped)} bytes ({len(swapped)/1024/1024:.2f} MB)")


if __name__ == "__main__":
    bit_file = sys.argv[1] if len(sys.argv) > 1 else "fpga/bitstream/snn_top.bit"
    bin_file = sys.argv[2] if len(sys.argv) > 2 else "fpga/bitstream/snn_top.bin"
    bit_to_bin(bit_file, bin_file)
