#!/bin/bash

set -e

echo "====================================="
echo "⚙️  Membangun Rust Backend (cargo build)..."
echo "====================================="
cargo build

echo ""
echo "====================================="
echo "🌐 Menjalankan Website Frontend (npm run dev)..."
echo "====================================="
npm run dev

