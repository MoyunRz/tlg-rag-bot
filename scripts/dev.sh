#!/bin/bash
# 本地开发启动脚本
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== 启动本地开发环境 ==="

# 检查 .env
if [ ! -f "$PROJECT_ROOT/.env" ]; then
    cp "$PROJECT_ROOT/.env.example" "$PROJECT_ROOT/.env"
    echo "已创建 .env，请编辑填入配置"
fi

# 检查 Chroma
echo "检查 Chroma..."
if curl -s http://127.0.0.1:8600/api/v1/heartbeat > /dev/null 2>&1; then
    echo "✓ Chroma 已运行"
else
    echo "✗ Chroma 未运行，请先启动: docker run -d --name chroma -p 8600:8000 chromadb/chroma"
    exit 1
fi

# 启动后端
echo "启动后端..."
cd "$PROJECT_ROOT/backend"
cargo run &
BACKEND_PID=$!

# 等待后端启动
sleep 3

# 启动前端
echo "启动前端..."
cd "$PROJECT_ROOT/frontend"
if [ ! -d "node_modules" ]; then
    npm install
fi
npm run dev &
FRONTEND_PID=$!

echo ""
echo "=== 服务已启动 ==="
echo "  后端: http://127.0.0.1:4000"
echo "  前端: http://127.0.0.1:5173"
echo ""
echo "按 Ctrl+C 停止所有服务"

# 等待信号
trap "echo '停止服务...'; kill $BACKEND_PID $FRONTEND_PID 2>/dev/null; exit" SIGINT SIGTERM
wait