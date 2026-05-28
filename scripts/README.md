# 脚本目录

## 脚本说明

| 脚本 | 说明 |
|---|---|
| `dev.sh` | 本地开发启动，一键启动前后端 + Chroma |
| `build-backend.sh` | 构建后端 release 二进制 |
| `build-frontend.sh` | 构建前端生产文件 |
| `package.sh` | 打包完整发行版（后端 + 前端 + 配置） |
| `deploy.sh` | Linux systemd 部署（需 root 权限） |
| `build-docker.sh` | Docker 镜像打包 |

## 快速使用

### 本地开发
```bash
bash scripts/dev.sh
```
启动后：
- 后端: http://127.0.0.1:4000
- 前端: http://127.0.0.1:5173

### 打包发行版
```bash
bash scripts/package.sh
```
产物输出到 `dist/` 目录：
- `bin/` — 后端二进制
- `web/` — 前端静态文件
- `.env` — 环境变量模板

### Docker 部署
```bash
bash scripts/build-docker.sh
docker-compose up -d
```

### Linux 部署（需 root）
```bash
sudo bash scripts/deploy.sh
```

## 依赖要求

- `dev.sh` — 需要 Docker 运行 Chroma
- `deploy.sh` — 需要 root 权限、systemd 支持
- `build-docker.sh` — 需要 Docker 和 docker-compose
- `build-backend.sh` — 需要 Rust 1.75+
- `build-frontend.sh` — 需要 Node.js 20+、npm 10+