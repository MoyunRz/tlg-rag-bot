<template>
  <div class="page-grid">
    <el-card class="grid-span-8 upload-card" shadow="never">
      <div class="card-header">
        <div class="card-icon upload-icon">
          <el-icon :size="20"><UploadFilled /></el-icon>
        </div>
        <div>
          <div class="card-title">上传知识库文档</div>
          <div class="card-desc">拖拽或点击上传，自动解析入库</div>
        </div>
      </div>
      <el-upload
        drag
        multiple
        :disabled="uploading"
        :http-request="uploadRequest"
        :accept="acceptedFileTypes"
        class="upload-zone"
      >
        <div class="upload-zone-inner">
          <div class="upload-icon-circle">
            <el-icon :size="28"><UploadFilled /></el-icon>
          </div>
          <div class="upload-zone-text">
            拖拽文件到此处，或 <span class="upload-link">点击选择文件</span>
          </div>
          <div class="upload-file-types">
            <span v-for="ft in fileTypes" :key="ft" class="file-type-tag">{{ ft }}</span>
          </div>
        </div>
      </el-upload>
    </el-card>

    <el-card class="grid-span-4" shadow="never">
      <div class="card-header">
        <div class="card-icon status-icon">
          <svg viewBox="0 0 20 20" fill="none" width="20" height="20">
            <path d="M10 2a8 8 0 100 16 8 8 0 000-16zm0 3a1 1 0 011 1v4l2.5 1.5a1 1 0 01-1 1.73L10 11.5V6a1 1 0 011-1z" fill="currentColor" opacity="0.8"/>
          </svg>
        </div>
        <div>
          <div class="card-title">索引状态</div>
          <div class="card-desc">当前知识库运行信息</div>
        </div>
      </div>
      <div class="status-list">
        <div class="status-row">
          <span class="status-label">Collection</span>
          <span class="status-value mono">{{ uploadSummary?.collection ?? 'tlg-rag-bot-model2vec' }}</span>
        </div>
        <div class="status-row">
          <span class="status-label">Embedding</span>
          <span class="status-value mono">potion-multilingual-128M</span>
        </div>
        <div class="status-row">
          <span class="status-label">Chunk 数量</span>
          <span class="status-value accent">{{ chunkCount }}</span>
        </div>
        <div class="status-row">
          <span class="status-label">最近入库</span>
          <span class="status-value">{{ uploadSummary ? `${uploadSummary.documents_processed} 文档 / ${uploadSummary.chunks_created} chunk` : '尚未上传' }}</span>
        </div>
        <div class="status-row">
          <span class="status-label">状态</span>
          <el-tag :type="uploadSummary?.status === 'success' ? 'success' : 'info'" size="small" effect="light" round>
            {{ uploadSummary?.status ?? 'idle' }}
          </el-tag>
        </div>
      </div>
    </el-card>

    <el-card class="grid-span-8 text-input-card" shadow="never">
      <div class="card-header">
        <div class="card-icon text-icon">
          <svg viewBox="0 0 20 20" fill="none" width="20" height="20">
            <path d="M4 4a2 2 0 012-2h8a2 2 0 012 2v12a2 2 0 01-2 2H6a2 2 0 01-2-2V4zm3 1h6v1.5H7V5zm0 3h6v1.5H7V8zm0 3h4v1.5H7V11z" fill="currentColor" opacity="0.8"/>
          </svg>
        </div>
        <div>
          <div class="card-title">直接输入文字</div>
          <div class="card-desc">粘贴文本内容，快速入库</div>
        </div>
      </div>
      <div class="text-input-body">
        <div class="source-name-field">
          <label class="field-label">来源名称</label>
          <el-input v-model="textSourceName" placeholder="例如：产品FAQ" maxlength="20" show-word-limit :disabled="textUploading" />
        </div>
        <div class="content-field">
          <label class="field-label">内容</label>
          <el-input
            v-model="textContent"
            type="textarea"
            :rows="5"
            placeholder="粘贴或输入知识文本"
            :disabled="textUploading"
          />
        </div>
        <div class="text-input-actions">
          <el-text type="info" size="small">{{ textContent.trim().length }} 字符</el-text>
          <el-button type="primary" :loading="textUploading" :disabled="!textSourceName.trim() || !textContent.trim()" @click="submitText">
            入库
          </el-button>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { UploadFilled } from '@element-plus/icons-vue'
import axios from 'axios'
import { ElMessage } from 'element-plus'
import type { UploadRequestOptions } from 'element-plus'
import { onMounted, ref } from 'vue'

import {
  fetchChunks,
  uploadKnowledgeBase,
  uploadText,
  type UploadResponse,
} from '../api/bot'

type UploadProgress = Parameters<UploadRequestOptions['onProgress']>[0]
type UploadError = Parameters<UploadRequestOptions['onError']>[0]

const acceptedFileTypes = '.txt,.md,.pdf,.png,.jpg,.jpeg,.doc,.docx'
const fileTypes = ['txt', 'md', 'pdf', 'png', 'jpg', 'doc', 'docx']
const uploading = ref(false)
const chunkCount = ref(0)
const uploadSummary = ref<UploadResponse | null>(null)

const loadChunks = async () => {
  try {
    const res = await fetchChunks({ page: 1, page_size: 1 })
    chunkCount.value = res.total
  } catch (error) {
    console.error(error)
  }
}

const uploadRequest = async (options: UploadRequestOptions) => {
  const formData = new FormData()
  formData.append(options.filename, options.file)
  uploading.value = true

  try {
    const response = await uploadKnowledgeBase(formData, (event) => {
      if (event.progress != null) {
        options.onProgress(toUploadProgress(event.progress * 100))
      }
    })

    options.onSuccess(response)
    uploadSummary.value = response

    if (response.documents_processed > 0) {
      await loadChunks()
    }

    const failedFiles = response.files.filter((file) => file.error)
    const failedMessage = failedFiles
      .map((file) => `${file.source_name}：${file.error}`)
      .join('；')
    const summaryMessage = `${response.documents_processed} 个文档，${response.chunks_created} 个 chunk`

    if (response.status === 'success') {
      ElMessage.success(`上传成功：${summaryMessage}`)
      return
    }

    if (response.status === 'partial_success') {
      ElMessage.warning(
        `部分成功：${summaryMessage}${failedFiles.length > 0 ? `；失败文件：${failedFiles.map((file) => file.source_name).join(', ')}` : ''}`,
      )
      return
    }

    ElMessage.error(failedMessage || response.message)
  } catch (error) {
    const message = resolveUploadError(error)
    options.onError(toUploadError(message, error))
    ElMessage.error(message)
  } finally {
    uploading.value = false
  }
}

const toUploadProgress = (percent: number) => {
  const event = new ProgressEvent('progress') as UploadProgress
  event.percent = percent
  return event
}

const toUploadError = (message: string, error: unknown) => {
  const uploadError = new Error(message) as UploadError
  uploadError.name = 'UploadError'
  uploadError.method = 'post'
  uploadError.url = '/api/kb/upload'
  uploadError.status = axios.isAxiosError(error) ? (error.response?.status ?? 500) : 500
  return uploadError
}

const resolveUploadError = (error: unknown) => {
  if (axios.isAxiosError(error)) {
    const responseMessage = error.response?.data?.message
    if (typeof responseMessage === 'string') {
      return responseMessage
    }

    if (typeof error.response?.data === 'string') {
      return error.response.data
    }
  }

  if (error instanceof Error) {
    return error.message
  }

  return '上传失败'
}

const textSourceName = ref('')
const textContent = ref('')
const textUploading = ref(false)

const submitText = async () => {
  const sourceName = textSourceName.value.trim()
  const text = textContent.value.trim()
  if (!sourceName || !text) return

  textUploading.value = true
  try {
    const response = await uploadText({ source_name: sourceName, text })
    uploadSummary.value = response
    ElMessage.success(`入库成功：${response.chunks_created} 个 chunk`)
    textSourceName.value = ''
    textContent.value = ''
    await loadChunks()
  } catch (error) {
    const message = resolveUploadError(error)
    ElMessage.error(message)
  } finally {
    textUploading.value = false
  }
}

onMounted(loadChunks)
</script>

<style scoped>
/* card header */
.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
}

.card-icon {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.upload-icon {
  background: linear-gradient(135deg, #eef2ff, #e0e7ff);
  color: #6366f1;
}

.text-icon {
  background: linear-gradient(135deg, #f0fdf4, #dcfce7);
  color: #16a34a;
}

.status-icon {
  background: linear-gradient(135deg, #fefce8, #fef3c7);
  color: #d97706;
}

.card-title {
  font-size: 15px;
  font-weight: 600;
  color: #111827;
  line-height: 1.3;
}

.card-desc {
  font-size: 12px;
  color: #9ca3af;
  margin-top: 1px;
}

/* upload zone */
.upload-zone :deep(.el-upload) {
  width: 100%;
}

.upload-zone :deep(.el-upload-dragger) {
  width: 100%;
  border: 2px dashed #e5e7eb;
  border-radius: 12px;
  padding: 32px 24px;
  background: #fafbfc;
  transition: all 0.2s ease;
}

.upload-zone :deep(.el-upload-dragger:hover) {
  border-color: #6366f1;
  background: #f5f3ff;
}

.upload-zone-inner {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.upload-icon-circle {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: linear-gradient(135deg, #eef2ff, #e0e7ff);
  color: #6366f1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.upload-zone-text {
  font-size: 14px;
  color: #6b7280;
}

.upload-link {
  color: #6366f1;
  font-weight: 500;
  cursor: pointer;
}

.upload-file-types {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  justify-content: center;
}

.file-type-tag {
  padding: 2px 8px;
  border-radius: 4px;
  background: #f3f4f6;
  color: #6b7280;
  font-size: 11px;
  font-weight: 500;
}

/* status list */
.status-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.status-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 0;
  border-bottom: 1px solid #f3f4f6;
}

.status-row:last-child {
  border-bottom: none;
}

.status-label {
  font-size: 13px;
  color: #9ca3af;
}

.status-value {
  font-size: 13px;
  color: #374151;
  font-weight: 500;
}

.status-value.mono {
  font-family: 'Cascadia Code', 'Fira Code', Consolas, monospace;
  font-size: 12px;
}

.status-value.accent {
  color: #6366f1;
  font-weight: 600;
  font-size: 15px;
}

/* text input card */
.text-input-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.source-name-field {
  width: 100%;
}

.field-label {
  display: block;
  font-size: 12px;
  color: #6b7280;
  margin-bottom: 6px;
  font-weight: 500;
}

.text-input-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
</style>
