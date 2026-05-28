#!/bin/bash
# 打包完整发行版（后端 + 前端 + 环境变量模板）
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_ROOT/dist"

echo "=== 打包发行版 ==="

# 构建前后端
bash "$SCRIPT_DIR/build-backend.sh"
bash "$SCRIPT_DIR/build-frontend.sh"

# 创建输出目录
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR/bin"
mkdir -p "$OUTPUT_DIR/web"

# 复制后端二进制
cp "$PROJECT_ROOT/backend/target/release/tlg-rag-bot" "$OUTPUT_DIR/bin/"

# 复制前端构建产物
cp -r "$PROJECT_ROOT/frontend/dist/"* "$OUTPUT_DIR/web/"

# 复制环境变量模板
cp "$PROJECT_ROOT/.env.example" "$OUTPUT_DIR/.env"

# 复制脚本
cp -r "$SCRIPT_DIR" "$OUTPUT_DIR/scripts"

# 复制 License 和 README
cp "$PROJECT_ROOT/LICENSE" "$OUTPUT_DIR/"
cp "$PROJECT_ROOT/README.md" "$OUTPUT_DIR/"

echo "打包完成，产物在 dist/"
ls -la "$OUTPUT_DIR"