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
