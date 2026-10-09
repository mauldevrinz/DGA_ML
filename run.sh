#!/bin/bash

set -e

echo "====================================="
echo "🔐 Meminta akses sudo (untuk FPGA Inference)..."
echo "====================================="
# Meminta password sudo di awal dan menyimpannya di cache
sudo -v
# Update timestamp sudo di background supaya tidak expire selama script jalan
while true; do sudo -n true; sleep 60; kill -0 "$$" || exit; done 2>/dev/null &

# Cleanup background processes on exit
cleanup() {
    echo ""
    echo "🛑 Shutting down..."
    if [ -n "${PYTHON_PID:-}" ]; then
        sudo kill "$PYTHON_PID" 2>/dev/null || true
    fi
    exit 0
}
trap cleanup INT TERM

echo "====================================="
echo "====================================="
echo "Ensuring SNN FPGA accelerator is active..."
echo "====================================="
if [ -e /sys/firmware/devicetree/base/axi/snn_top@B0000000 ]; then
  echo "SNN overlay already active; skipping loader."
else
  echo "SNN overlay not active; loading bitstream..."
  sudo ./fpga/load_snn.sh
fi
echo ""

echo "⚙️  Membangun Rust Backend (cargo build)..."
echo "====================================="
# Asumsi Cargo.toml ada di folder persistence_rs atau root
if [ -f "Cargo.toml" ] || [ -f "persistence_rs/Cargo.toml" ]; then
    (cd persistence_rs 2>/dev/null || true; cargo build)
else
    echo "⚠️  Cargo.toml tidak ditemukan, skip build Rust."
fi

echo ""
echo "====================================="
echo "📊 Cek InfluxDB service..."
echo "====================================="
if curl -sf http://127.0.0.1:8086/health > /dev/null; then
    echo "InfluxDB OK (127.0.0.1:8086)"
else
    echo "⚠️  InfluxDB tidak merespons di 127.0.0.1:8086 — data sensor tetap tersimpan di SQLite,"
    echo "    tapi tidak akan masuk ke InfluxDB. Cek: sudo systemctl status influxdb"
fi

echo ""
echo "====================================="
echo "🐍 Menjalankan Python WebSocket Server (serial_ws.py) [SUDO]..."
echo "====================================="
if ! python3 -c "import numpy, websockets, usb, requests" >/dev/null 2>&1; then
    echo "❌ Dependensi Python belum lengkap."
    echo "   Jalankan: python3 -m pip install -r requirements.txt"
    exit 1
fi
USER_SITE=$(python3 -m site --user-site)
if pgrep -f "[s]erial_ws.py" >/dev/null; then
  echo "WebSocket server sudah berjalan; memakai instance yang ada."
  PYTHON_PID=$(pgrep -f "[s]erial_ws.py" | head -1)
else
  mkdir -p logs
  echo "[$(date -Is)] Starting serial_ws.py" >> logs/serial_ws.log
  sudo PYTHONUNBUFFERED=1 PYTHONPATH="$USER_SITE:$PYTHONPATH" python3 serial_ws.py >> logs/serial_ws.log 2>&1 &
  PYTHON_PID=$!
fi
sleep 2
if ! sudo kill -0 "$PYTHON_PID" 2>/dev/null; then
    echo "❌ Python WebSocket Server gagal dijalankan."
    exit 1
fi

echo ""
echo "====================================="
echo "🌐 Menjalankan Website Frontend (npm run dev)..."
echo "====================================="
npm run dev

cleanup
