#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || ! -f "$1" ]]; then
  echo "Usage: cargo run --release (runner expects a built ELF path)" >&2
  exit 2
fi

elf="$1"
hex="${elf}.hex"
cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
export PATH="$cargo_bin:$PATH"

objcopy="$(command -v rust-objcopy || true)"
if [[ -z "$objcopy" && -x "$cargo_bin/rust-objcopy" ]]; then
  objcopy="$cargo_bin/rust-objcopy"
fi
if [[ -z "$objcopy" ]]; then
  echo "Missing rust-objcopy. Install it with: cargo install cargo-binutils; rustup component add llvm-tools" >&2
  exit 127
fi

loader="$(command -v teensy_loader_cli || true)"
if [[ -z "$loader" ]]; then
  echo "Missing teensy_loader_cli. Install Teensy Loader CLI and add it to PATH; no firmware was flashed." >&2
  exit 127
fi

"$objcopy" -O ihex -R .defmt.end "$elf" "$hex"
sed -i 's/\r$//' "$hex"
exec "$loader" --mcu=TEENSY41 -w -v "$hex"
