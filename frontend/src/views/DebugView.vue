<template>
  <div class="page-grid">
    <el-card class="grid-span-12">
      <template #header>
        <div class="section-title">在线调试</div>
      </template>
      <p class="section-description">
        这里会调用 `/api/debug/query`，优先展示整理后的答案、引用来源和耗时，并保留原始调试数据。
      </p>
      <el-form label-position="top" @submit.prevent>
        <el-form-item label="测试问题">
          <el-input v-model="question" type="textarea" :rows="4" placeholder="输入测试问题" />
        </el-form-item>
        <el-button type="primary" :loading="loading" @click="handleRun">运行调试</el-button>
      </el-form>
    </el-card>

    <el-card class="grid-span-8">
      <template #header>
        <div class="debug-toolbar">
          <div>
            <div class="section-title debug-toolbar__title">答案</div>
            <div class="debug-toolbar__subtitle">后端返回的 Markdown 答案会在这里渲染。</div>
          </div>
          <div class="debug-toolbar__actions">
            <el-button :disabled="!result" @click="handleCopyAnswer">复制答案 Markdown</el-button>
            <el-button :disabled="!result" @click="handleExportMarkdown">导出 .md</el-button>
          </div>
        </div>
      </template>

      <el-empty v-if="!result" description="运行一次调试查询后，在这里查看整理后的答案。" />
      <article v-else class="markdown-answer" v-html="renderedAnswer"></article>
    </el-card>

    <el-card class="grid-span-4">
      <template #header>
        <div class="section-title">本次信息</div>
      </template>
      <el-descriptions :column="1" border>
        <el-descriptions-item label="问题">{{ result?.question ?? '-' }}</el-descriptions-item>
        <el-descriptions-item label="格式">{{ result?.answer_format ?? 'plain' }}</el-descriptions-item>
        <el-descriptions-item label="引用数">{{ citations.length }}</el-descriptions-item>
        <el-descriptions-item label="召回数">{{ result?.retrieval_count ?? 0 }}</el-descriptions-item>
        <el-descriptions-item label="Top K">{{ result?.top_k ?? '-' }}</el-descriptions-item>
        <el-descriptions-item label="阈值">{{ result?.score_threshold ?? '-' }}</el-descriptions-item>
        <el-descriptions-item label="Provider">{{ result?.provider ?? '-' }}</el-descriptions-item>
        <el-descriptions-item label="Model">{{ result?.model ?? '-' }}</el-descriptions-item>
        <el-descriptions-item label="Embed">{{ formatMs(result?.timings_ms?.embed) }}</el-descriptions-item>
        <el-descriptions-item label="Retrieve">{{ formatMs(result?.timings_ms?.retrieve) }}</el-descriptions-item>
        <el-descriptions-item label="Generate">{{ formatMs(result?.timings_ms?.generate) }}</el-descriptions-item>
        <el-descriptions-item label="Total">{{ formatMs(result?.timings_ms?.total) }}</el-descriptions-item>
      </el-descriptions>
    </el-card>

    <el-card class="grid-span-12">
      <template #header>
        <div class="section-title">引用来源</div>
      </template>

      <el-empty v-if="!citations.length" description="本次回答没有可展示的引用来源。" />
      <div v-else class="citation-list">
        <article v-for="citation in citations" :key="`${citation.document_id}-${citation.chunk_index}`" class="citation-card">
          <div class="citation-card__header">
            <div>
              <div class="citation-card__title">[{{ citation.index }}] {{ citation.source_name }}</div>
              <div class="citation-card__meta">
                document {{ citation.document_id }} · chunk {{ citation.chunk_index }}
              </div>
            </div>
            <el-tag type="info" effect="plain">score {{ formatScore(citation.score) }}</el-tag>
          </div>
          <p class="citation-card__excerpt">{{ citation.excerpt }}</p>
          <div v-if="citation.tags.length" class="citation-card__tags">
            <el-tag v-for="tag in citation.tags" :key="tag" size="small" effect="plain">{{ tag }}</el-tag>
          </div>
        </article>
      </div>
    </el-card>

    <el-card class="grid-span-12">
      <template #header>
        <div class="debug-toolbar">
          <div>
            <div class="section-title debug-toolbar__title">深度调试</div>
            <div class="debug-toolbar__subtitle">按需展开召回 chunk 和原始 JSON。</div>
          </div>
          <div class="debug-toolbar__actions">
            <el-button :disabled="!result" @click="handleCopyJson">复制原始 JSON</el-button>
          </div>
        </div>
      </template>

      <el-collapse v-model="activePanels">
        <el-collapse-item name="chunks" title="召回 chunk">
          <el-empty
            v-if="!result?.retrieved_chunk_items?.length"
            description="本次没有可展示的召回 chunk。"
          />
          <el-table v-else :data="result.retrieved_chunk_items">
            <el-table-column prop="source_name" label="来源" width="220" />
            <el-table-column prop="chunk_index" label="Chunk" width="90" />
            <el-table-column prop="score" label="Score" width="110">
              <template #default="scope">
                {{ formatScore(scope.row.score) }}
              </template>
            </el-table-column>
            <el-table-column prop="distance" label="Distance" width="110">
              <template #default="scope">
                {{ formatScore(scope.row.distance) }}
              </template>
            </el-table-column>
            <el-table-column prop="text" label="内容" />
          </el-table>
        </el-collapse-item>

        <el-collapse-item name="json" title="原始 JSON">
          <pre class="code-block">{{ rawJson }}</pre>
        </el-collapse-item>
      </el-collapse>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import DOMPurify from 'dompurify'
import MarkdownIt from 'markdown-it'
import { ElMessage } from 'element-plus'
import hljs from 'highlight.js'
import { computed, ref } from 'vue'

import { runDebugQuery, type DebugResponse } from '../api/bot'

const escapeHtml = (value: string) =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

const markdown = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
  highlight(code: string, language: string): string {
    if (language && hljs.getLanguage(language)) {
      return `<pre><code class="hljs">${hljs.highlight(code, { language }).value}</code></pre>`
    }

    return `<pre><code class="hljs">${escapeHtml(code)}</code></pre>`
  },
})

const question = ref('')
const loading = ref(false)
const result = ref<DebugResponse | null>(null)
const activePanels = ref<string[]>([])

const citations = computed(() => result.value?.citations ?? [])
const renderedAnswer = computed(() => {
  if (!result.value) {
    return ''
  }

  return DOMPurify.sanitize(markdown.render(result.value.final_answer))
})
const rawJson = computed(() => JSON.stringify(result.value, null, 2))

const handleRun = async () => {
  loading.value = true

  try {
    result.value = await runDebugQuery(question.value)
    activePanels.value = []
  } catch (error) {
    console.error(error)
    ElMessage.error('调试接口调用失败')
  } finally {
    loading.value = false
  }
}

const handleCopyAnswer = async () => {
  if (!result.value) {
    return
  }

  await copyText(result.value.final_answer, '已复制答案 Markdown')
}

const handleCopyJson = async () => {
  if (!result.value) {
    return
  }

  await copyText(rawJson.value, '已复制原始 JSON')
}

const handleExportMarkdown = () => {
  if (!result.value) {
    return
  }

  const content = buildMarkdownExport(result.value)
  const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = `debug-answer-${Date.now()}.md`
  link.click()
  URL.revokeObjectURL(url)
  ElMessage.success('已导出 Markdown 文件')
}

const copyText = async (text: string, successMessage: string) => {
  try {
    await navigator.clipboard.writeText(text)
    ElMessage.success(successMessage)
  } catch (error) {
    console.error(error)
    ElMessage.error('复制失败')
  }
}

const buildMarkdownExport = (payload: DebugResponse) => {
  const citationLines = (payload.citations ?? []).map(
    (citation) =>
      `- [${citation.index}] ${citation.source_name} · document ${citation.document_id} · chunk ${citation.chunk_index} · score ${formatScore(citation.score)}\n  ${citation.excerpt}`,
  )

  const timings = payload.timings_ms
    ? `- Embed: ${formatMs(payload.timings_ms.embed)}\n- Retrieve: ${formatMs(payload.timings_ms.retrieve)}\n- Generate: ${formatMs(payload.timings_ms.generate)}\n- Total: ${formatMs(payload.timings_ms.total)}`
    : '- 无 timings'

  return [
    '# 调试答案',
    '',
    `## 问题`,
    payload.question,
    '',
    '## 回答',
    payload.final_answer,
    '',
    '## 引用',
    citationLines.length ? citationLines.join('\n') : '- 无引用',
    '',
    '## Timings',
    timings,
  ].join('\n')
}

const formatMs = (value?: number) => (value == null ? '-' : `${value} ms`)
const formatScore = (value: number) => value.toFixed(3)
</script>

<style scoped>
.debug-toolbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.debug-toolbar__title {
  margin-bottom: 4px;
}

.debug-toolbar__subtitle {
  color: #6b7280;
  font-size: 13px;
  line-height: 1.6;
}

.debug-toolbar__actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.markdown-answer {
  color: #111827;
  line-height: 1.8;
  word-break: break-word;
}

.markdown-answer :deep(h1),
.markdown-answer :deep(h2),
.markdown-answer :deep(h3) {
  margin: 1.2em 0 0.6em;
  line-height: 1.4;
}

.markdown-answer :deep(h1:first-child),
.markdown-answer :deep(h2:first-child),
.markdown-answer :deep(h3:first-child) {
  margin-top: 0;
}

.markdown-answer :deep(p),
.markdown-answer :deep(ul),
.markdown-answer :deep(ol),
.markdown-answer :deep(table),
.markdown-answer :deep(pre) {
  margin: 0 0 16px;
}

.markdown-answer :deep(table) {
  width: 100%;
  border-collapse: collapse;
}

.markdown-answer :deep(th),
.markdown-answer :deep(td) {
  border: 1px solid #e5e7eb;
  padding: 10px 12px;
  text-align: left;
  vertical-align: top;
}

.markdown-answer :deep(th) {
  background: #f8fafc;
}

.markdown-answer :deep(code) {
  font-family: 'Cascadia Code', 'SFMono-Regular', Consolas, monospace;
}

.markdown-answer :deep(pre) {
  border-radius: 12px;
  overflow-x: auto;
}

.markdown-answer :deep(pre code) {
  display: block;
  padding: 16px;
  line-height: 1.6;
}

.citation-list {
  display: grid;
  gap: 12px;
}

.citation-card {
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  padding: 16px;
  background: #fff;
}

.citation-card__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.citation-card__title {
  font-size: 16px;
  font-weight: 600;
  color: #111827;
}

.citation-card__meta {
  margin-top: 4px;
  font-size: 13px;
  color: #6b7280;
}

.citation-card__excerpt {
  margin: 12px 0;
  color: #374151;
  line-height: 1.7;
  white-space: pre-wrap;
}

.citation-card__tags {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
</style>
