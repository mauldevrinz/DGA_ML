#!/bin/bash

set -e

# Cleanup background processes on exit
cleanup() {
    echo ""
    echo "🛑 Shutting down..."
    kill $PYTHON_PID 2>/dev/null
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
python3 serial_ws.py &
PYTHON_PID=$!
sleep 2

echo ""
echo "====================================="
echo "🌐 Menjalankan Website Frontend (npm run dev)..."
echo "====================================="
npm run dev

cleanup
