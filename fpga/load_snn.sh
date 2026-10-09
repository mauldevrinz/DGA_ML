#!/bin/bash
#
# load_snn.sh — Load the SNN bitstream onto the Kria KV260 FPGA
#
# Usage:  sudo ./fpga/load_snn.sh
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BIT_FILE="$SCRIPT_DIR/bitstream/snn_top.bit"
BIN_FILE="$SCRIPT_DIR/bitstream/snn_top.bin"

FIRMWARE_DIR="/lib/firmware/xilinx/snn_accel"
FIRMWARE_NAME="snn_accel"

echo "=== SNN FPGA Loader for Kria KV260 ==="
echo ""

# -------------------------------------------------------
# Step 1: Convert .bit → .bin (if not already done)
# -------------------------------------------------------
if [ ! -f "$BIN_FILE" ]; then
    echo "[1/4] Converting .bit → .bin ..."
    python3 "$SCRIPT_DIR/convert_bit_to_bin.py" "$BIT_FILE" "$BIN_FILE"
else
    echo "[1/4] .bin file already exists, skipping conversion."
fi

# -------------------------------------------------------
# Step 2: Compile device-tree overlay (if not already done)
# -------------------------------------------------------
DTBO_FILE="$SCRIPT_DIR/snn_accel.dtbo"
if [ ! -f "$DTBO_FILE" ]; then
    echo "[2/4] Compiling device-tree overlay ..."
    dtc -@ -O dtb -o "$DTBO_FILE" "$SCRIPT_DIR/snn_accel.dts"
else
    echo "[2/4] .dtbo already exists, skipping compilation."
fi

# -------------------------------------------------------
# Step 3: Install firmware to /lib/firmware/xilinx/
# -------------------------------------------------------
echo "[3/4] Installing firmware to $FIRMWARE_DIR ..."
mkdir -p "$FIRMWARE_DIR"
cp "$BIN_FILE" "$FIRMWARE_DIR/${FIRMWARE_NAME}.bin"
cp "$DTBO_FILE" "$FIRMWARE_DIR/${FIRMWARE_NAME}.dtbo"
cat > "$FIRMWARE_DIR/shell.json" << 'EOF'
{
    "shell_type" : "XRT_FLAT",
    "num_slots": "1"
}
EOF
echo "  Installed: ${FIRMWARE_NAME}.bin, ${FIRMWARE_NAME}.dtbo, shell.json"

# -------------------------------------------------------
# Step 4: Unload current app and load SNN
# -------------------------------------------------------
echo "[4/4] Loading SNN accelerator via xmutil ..."

# Unload the current app (if any)
CURRENT_APP=$(xmutil listapps 2>/dev/null | head -1 || true)
if [ -n "$CURRENT_APP" ] && [ "$CURRENT_APP" != "$FIRMWARE_NAME" ]; then
    echo "  Unloading current app: $CURRENT_APP"
    xmutil unloadapp || true
fi

# Load SNN
xmutil loadapp "$FIRMWARE_NAME"

echo ""
echo "=== Done! ==="
echo ""

# Verify
FPGA_STATE=$(cat /sys/class/fpga_manager/fpga0/state 2>/dev/null || echo "unknown")
echo "FPGA state: $FPGA_STATE"

# Check if UIO device appeared
if ls /dev/uio* 1>/dev/null 2>&1; then
    echo "UIO devices:"
    for uio in /dev/uio*; do
        name=$(cat /sys/class/uio/$(basename $uio)/name 2>/dev/null || echo "unknown")
        echo "  $uio -> $name"
    done
else
    echo ""
    echo "NOTE: No UIO device found. The driver will use /dev/mem instead."
    echo "      Make sure to run the inference script with sudo."
fi

echo ""
echo "Next: Run inference with:"
echo "  sudo python3 fpga/snn_driver.py"
