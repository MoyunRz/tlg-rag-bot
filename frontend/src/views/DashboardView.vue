<template>
  <div class="page-grid">
    <el-card class="grid-span-4">
      <div class="stat-value">1</div>
      <div class="stat-label">知识库文档</div>
    </el-card>
    <el-card class="grid-span-4">
      <div class="stat-value">1</div>
      <div class="stat-label">FAQ 条目</div>
    </el-card>
    <el-card class="grid-span-4">
      <div class="stat-value">1</div>
      <div class="stat-label">最近会话</div>
    </el-card>

    <el-card class="grid-span-8">
      <template #header>
        <div class="section-title">当前交付范围</div>
      </template>
      <p class="section-description">
        当前已经接通 Telegram Long Polling、知识库上传分块入库、gllm 向量编码、Chroma 检索，以及 Minimax 驱动的基础 RAG 问答链路。
      </p>
      <el-space wrap>
        <el-tag type="success">Long Polling</el-tag>
        <el-tag type="success">Chroma</el-tag>
        <el-tag type="warning">gllm CPU</el-tag>
        <el-tag>Minimax</el-tag>
      </el-space>
    </el-card>

    <el-card class="grid-span-4">
      <template #header>
        <div class="section-title">环境变量</div>
      </template>
      <ul>
        <li>TELEGRAM_BOT_TOKEN</li>
        <li>CHROMA_URL</li>
        <li>LLM_API_KEY</li>
        <li>LLM_MODEL</li>
        <li>RAG_TOP_K</li>
      </ul>
    </el-card>
  </div>
</template>
