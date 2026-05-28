# tlg-rag-bot 项目大纲

## 1. 项目目标

构建一个基于 Telegram 的自动问答客服机器人，支持：
- Telegram webhook 接收用户消息
- 基于 RAG 的知识库问答
- FAQ 优先命中与兜底回答
- 后台页面管理知识库、FAQ、会话记录与在线调试

技术栈：
- Bot: Rust + teloxide
- Backend: axum
- Frontend: Vue 3 + Vite + Element Plus
- Vector DB: Chroma
- Embedding: `model2vec-rs`（默认使用本地/静态 embedding 模型）
- 回答模型：保留可插拔配置

## 2. 系统架构

建议目录：

```text
tlg-rag-bot/
  docs/
    overview.md
  backend/
    Cargo.toml
    src/
      main.rs
      app.rs
      config.rs
      error.rs
      routes/
      services/
      repos/
      models/
      ingest/
      llm/
  frontend/
    package.json
    vite.config.ts
    src/
      api/
      router/
      views/
      components/
```

模块职责：
- `backend/routes`: 对外 HTTP 接口与 Telegram webhook
- `backend/services`: RAG、FAQ、会话、知识库处理
- `backend/repos`: FAQ / 会话 / 文档元数据存取
- `backend/ingest`: 文档解析、切分、入库
- `backend/llm`: 回答模型抽象
- `frontend/views`: 后台业务页面

## 3. Telegram 消息处理流程

1. Telegram 将消息推送到 webhook
2. `axum` 接收更新并交给 `teloxide` 处理
3. 提取用户问题与上下文信息
4. 先执行 FAQ 命中逻辑
5. 未命中则进入 RAG 检索
6. 生成最终回答并回发 Telegram
7. 保存会话、命中片段、答案与调试信息

## 4. RAG 检索与回答流程

1. 管理端上传文档
2. 后端解析为纯文本
3. 文本按 chunk 规则切分
4. 使用本地 `model2vec-rs` CPU embedding 生成向量
5. 将 chunk 与向量写入 Chroma
6. 用户提问时生成 query embedding
7. 从 Chroma 检索 top-k 相关片段
8. 组装 prompt + context
9. 调用回答模型生成回复
10. 记录 trace，便于后台调试查看

## 5. Chroma / Embedding 方案

### Chroma
- 作为外部向量库服务运行
- 按 collection 管理知识库
- 存储 chunk 文本、向量、来源、标签、更新时间

### Embedding
- 固定使用 `model2vec-rs` 本地静态 embedding
- 服务层单独封装 `embed_service`
- 对上传文档和用户 query 统一走相同 embedding 流程
- 切换 embedding 模型家族时必须使用新的 Chroma collection，并重新导入知识库

### 数据建议
每个 chunk 至少保存：
- `id`
- `document_id`
- `text`
- `source_name`
- `tags`
- `embedding`
- `created_at`

## 6. 后端 API 设计

### 基础接口
- `GET /api/health`
- `POST /telegram/webhook`

### 知识库
- `POST /api/kb/upload`
- `GET /api/kb/chunks`
- `POST /api/kb/reindex`

### FAQ
- `GET /api/faqs`
- `POST /api/faqs`
- `PUT /api/faqs/:id`
- `DELETE /api/faqs/:id`

### 会话
- `GET /api/conversations`
- `GET /api/conversations/:id`

### 在线调试
- `POST /api/debug/query`

## 7. 前端页面设计

### Dashboard
- 知识库文档数
- FAQ 数量
- 最近会话数
- 最近一次入库状态

### KnowledgeBase
- 上传文档
- 查看文档与 chunk 列表
- 触发重建索引

### FaqManager
- FAQ 列表
- 新增 / 编辑 / 删除
- 标签与启停状态管理

### Conversations
- 会话列表
- 查看用户问题、命中片段、最终回答、时间
- 支持筛选与详情抽屉

### DebugPanel
- 手动输入问题
- 查看 FAQ 是否命中
- 查看召回 chunk
- 查看 prompt 与最终答案

## 8. 数据模型

建议首版至少包含：

### FAQ
- `id`
- `question`
- `answer`
- `tags`
- `enabled`
- `created_at`

### Conversation
- `id`
- `user_id`
- `chat_id`
- `question`
- `answer`
- `retrieved_chunks`
- `trace`
- `created_at`

### Document
- `id`
- `name`
- `status`
- `chunk_count`
- `created_at`

## 9. 部署与配置

建议环境变量：
- `TELEGRAM_ENABLED`
- `TELEGRAM_BOT_TOKEN`
- `TELEGRAM_WEBHOOK_URL`
- `TELEGRAM_WEBHOOK_SECRET`
- `CHROMA_URL`
- `CHROMA_COLLECTION`
- `LLM_PROVIDER`
- `LLM_BASE_URL`
- `LLM_API_KEY`
- `RAG_TOP_K`
- `RAG_SCORE_THRESHOLD`

部署建议：
- `backend` 独立部署
- `frontend` 构建后由静态服务托管
- Chroma 独立部署
- 通过反向代理暴露 webhook 与后台接口

## 10. MVP 范围与后续迭代

### MVP
- Telegram webhook 收发消息
- 文档上传 + chunk + embedding + Chroma 入库
- FAQ 管理
- 会话查看
- 在线调试页

### 后续迭代
- 多知识库 / 多租户
- 管理员登录鉴权
- 命中评分与反馈闭环
- 敏感词与安全审计
- 自动重排序与引用展示

## 11. 实施顺序

1. 创建 docs 大纲
2. 初始化 `backend` Rust + axum 项目
3. 初始化 `frontend` Vite + Vue 3 + Element Plus 项目
4. 打通 health 与 webhook
5. 接入 Chroma 与 `model2vec-rs` embedding
6. 完成知识库上传链路
7. 完成 FAQ 管理链路
8. 完成会话记录与查看
9. 完成在线调试页
10. 联调 Telegram 问答闭环
