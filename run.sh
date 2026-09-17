#!/bin/bash

set -e

# Cleanup background processes on exit
cleanup() {
    echo ""
    echo "🛑 Shutting down..."
    if [ -n "${PYTHON_PID:-}" ]; then
        kill "$PYTHON_PID" 2>/dev/null || true
    fi
    exit 0
}
trap cleanup INT TERM

echo "====================================="
echo "⚙️  Membangun Rust Backend (cargo build)..."
echo "====================================="
cargo build

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
echo "🐍 Menjalankan Python WebSocket Server (serial_ws.py)..."
echo "====================================="
if ! python3 -c "import numpy, skfuzzy, websockets, usb, requests" >/dev/null 2>&1; then
    echo "❌ Dependensi Python belum lengkap."
    echo "   Jalankan: python3 -m pip install -r requirements.txt"
    exit 1
fi
python3 serial_ws.py &
PYTHON_PID=$!
sleep 2
if ! kill -0 "$PYTHON_PID" 2>/dev/null; then
    echo "❌ Python WebSocket Server gagal dijalankan."
    exit 1
fi

echo ""
echo "====================================="
echo "🌐 Menjalankan Website Frontend (npm run dev)..."
echo "====================================="
npm run dev

cleanup
