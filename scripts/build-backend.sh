#!/bin/bash
# 构建后端 release
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BACKEND_DIR="$PROJECT_ROOT/backend"

echo "=== 构建后端 ==="
cd "$BACKEND_DIR"
cargo build --release
echo "构建完成，产物在 backend/target/release/tlg-rag-bot"