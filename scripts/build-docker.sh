#!/bin/bash
# Docker 打包脚本
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Docker 打包 ==="

# 构建前端
bash "$SCRIPT_DIR/build-frontend.sh"

# 构建 Docker 镜像
cd "$PROJECT_ROOT"
docker build -f backend/Dockerfile -t tlg-rag-bot:latest .
docker build -f Dockerfile.frontend -t tlg-rag-bot-frontend:latest .

echo "=== 镜像构建完成 ==="
echo "  后端镜像: tlg-rag-bot:latest"
echo "  前端镜像: tlg-rag-bot-frontend:latest"
echo ""
echo "启动方式:"
echo "  docker-compose up -d"