#!/bin/bash
# 构建前端生产文件
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FRONTEND_DIR="$PROJECT_ROOT/frontend"

echo "=== 构建前端 ==="
cd "$FRONTEND_DIR"

if [ ! -d "node_modules" ]; then
    echo "安装依赖..."
    npm install
fi

npm run build
echo "构建完成，产物在 frontend/dist/"