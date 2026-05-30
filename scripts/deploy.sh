#!/bin/bash
# Linux systemd 部署脚本
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
INSTALL_DIR="/opt/tlg-rag-bot"
SERVICE_NAME="tlg-rag-bot"
NGINX_PORT=${NGINX_PORT:-4001}  # 默认 80，可通过环境变量修改

echo "=== Linux 部署脚本 ==="

# 需要 root 权限
if [ "$EUID" -ne 0 ]; then
    echo "请使用 root 权限运行: sudo bash $0"
    exit 1
fi

# 打包（如还未打包）
if [ ! -d "$PROJECT_ROOT/dist" ]; then
    echo "打包中..."
    bash "$SCRIPT_DIR/package.sh"
fi

# 创建安装目录
mkdir -p "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR/web"

# 复制文件
cp -r "$PROJECT_ROOT/dist/"* "$INSTALL_DIR/"
cp -r "$PROJECT_ROOT/dist/bin" "$INSTALL_DIR/"
cp -r "$PROJECT_ROOT/dist/web" "$INSTALL_DIR/"

# 创建 nginx 配置
cat > /etc/nginx/sites-available/tlg-frontend <<EOF
server {
    listen ${NGINX_PORT};
    server_name _;

    root ${INSTALL_DIR}/web;
    index index.html;

    # 前端静态文件
    location / {
        try_files \$uri \$uri/ /index.html;
    }

    # API 代理到后端
    location /api/ {
        proxy_pass http://127.0.0.1:4000;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
    }
}
EOF

# 启用 nginx 配置
ln -sf /etc/nginx/sites-available/tlg-frontend /etc/nginx/sites-enabled/tlg-frontend
rm -f /etc/nginx/sites-enabled/default 2>/dev/null || true

# 测试并重启 nginx
nginx -t && systemctl reload nginx

# 创建 .env
if [ ! -f "$INSTALL_DIR/.env" ]; then
    cp "$PROJECT_ROOT/.env.example" "$INSTALL_DIR/.env"
    echo "请编辑 $INSTALL_DIR/.env 填入配置"
fi

# 创建 systemd service
cat > /etc/systemd/system/${SERVICE_NAME}.service <<EOF
[Unit]
Description=tlg-rag-bot Backend
After=network.target

[Service]
WorkingDirectory=$INSTALL_DIR
EnvironmentFile=$INSTALL_DIR/.env
ExecStart=$INSTALL_DIR/bin/tlg-rag-bot
Restart=always
RestartSec=5
User=root

[Install]
WantedBy=multi-user.target
EOF

# 重载 systemd 并启动
systemctl daemon-reload
systemctl enable ${SERVICE_NAME}
systemctl restart ${SERVICE_NAME}

echo "=== 部署完成 ==="
echo "后端服务: $INSTALL_DIR/bin/tlg-rag-bot"
echo "前端文件: $INSTALL_DIR/web/"
echo "前端端口: ${NGINX_PORT}"
echo "配置文件: $INSTALL_DIR/.env"
echo ""
echo "常用命令:"
echo "  systemctl restart ${SERVICE_NAME}  # 重启服务"
echo "  systemctl status ${SERVICE_NAME}    # 查看状态"
echo "  journalctl -u ${SERVICE_NAME} -f   # 查看日志"
echo "  NGINX_PORT=8080 sudo bash deploy.sh  # 修改前端端口为 8080"