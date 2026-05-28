<template>
  <div class="chunk-list-page">
    <div class="page-toolbar">
      <el-select v-model="sourceFilter" placeholder="按来源筛选" clearable filterable style="width: 200px" @change="onFilterChange">
        <el-option v-for="s in sources" :key="s" :label="s" :value="s" />
      </el-select>
      <el-button :loading="loading" @click="loadChunks">刷新</el-button>
      <el-button
        v-if="selectedIds.length > 0"
        type="danger"
        :loading="deleting"
        @click="deleteSelected"
      >
        删除已选 ({{ selectedIds.length }})
      </el-button>
      <el-button
        v-if="sourceFilter && !deleting"
        type="danger"
        plain
        @click="deleteBySource"
      >
        删除此来源所有 Chunk
      </el-button>
    </div>
    <el-card shadow="never">
      <el-table v-loading="loading" :data="rows" stripe @selection-change="onSelectionChange">
        <el-table-column type="selection" width="40" />
        <el-table-column label="ID" width="180">
          <template #default="scope">
            <el-text truncated class="mono-text">{{ scope.row.id }}</el-text>
          </template>
        </el-table-column>
        <el-table-column prop="source_name" label="来源" width="180">
          <template #default="scope">
            <el-tag size="small" type="info" effect="plain">{{ scope.row.source_name }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="内容">
          <template #default="scope">
            <div class="chunk-text-cell">
              <span class="chunk-text">{{ scope.row.text }}</span>
              <el-popover v-if="scope.row.text.length > 120" placement="top" :width="480" trigger="click">
                <template #reference>
                  <el-button link type="primary" size="small" class="expand-btn">展开</el-button>
                </template>
                <div class="chunk-full-text">{{ scope.row.text }}</div>
              </el-popover>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="标签" width="160">
          <template #default="scope">
            <template v-if="scope.row.tags.length">
              <el-tag v-for="tag in scope.row.tags" :key="tag" size="small" style="margin-right: 4px">{{ tag }}</el-tag>
            </template>
            <el-text v-else type="info">-</el-text>
          </template>
        </el-table-column>
      </el-table>
      <el-pagination
        v-model:current-page="currentPage"
        v-model:page-size="pageSize"
        :total="total"
        :page-sizes="[10, 20, 50, 100]"
        layout="total, sizes, prev, pager, next, jumper"
        style="margin-top: 16px; justify-content: flex-end;"
        @current-change="loadChunks"
        @size-change="loadChunks"
      />
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ElMessage, ElMessageBox } from 'element-plus'
import { onMounted, ref } from 'vue'

import { deleteChunks, fetchChunks, fetchSources, type ChunkItem } from '../api/bot'

const loading = ref(false)
const deleting = ref(false)
const rows = ref<ChunkItem[]>([])
const total = ref(0)
const currentPage = ref(1)
const pageSize = ref(20)
const sources = ref<string[]>([])
const sourceFilter = ref('')
const selectedIds = ref<string[]>([])

const loadChunks = async () => {
  loading.value = true
  try {
    const res = await fetchChunks({ page: currentPage.value, page_size: pageSize.value, source: sourceFilter.value || undefined })
    rows.value = res.items
    total.value = res.total
  } catch (error) {
    console.error(error)
    ElMessage.error('加载 chunk 失败')
  } finally {
    loading.value = false
  }
}

const loadSources = async () => {
  try {
    sources.value = await fetchSources()
  } catch (error) {
    console.error(error)
  }
}

const onFilterChange = () => {
  currentPage.value = 1
  loadChunks()
}

const onSelectionChange = (rows: ChunkItem[]) => {
  selectedIds.value = rows.map(r => r.id)
}

const deleteSelected = async () => {
  if (selectedIds.value.length === 0) return
  try {
    await ElMessageBox.confirm(`确认删除 ${selectedIds.value.length} 个 chunk？`, '批量删除', { type: 'warning' })
    deleting.value = true
    const res = await deleteChunks({ ids: selectedIds.value })
    ElMessage.success(`已删除 ${res.deleted} 个 chunk`)
    selectedIds.value = []
    await loadChunks()
    await loadSources()
  } catch (error) {
    if (error !== 'cancel') {
      console.error(error)
      ElMessage.error('删除失败')
    }
  } finally {
    deleting.value = false
  }
}

const deleteBySource = async () => {
  if (!sourceFilter.value) return
  try {
    await ElMessageBox.confirm(`确认删除来源 "${sourceFilter.value}" 的所有 chunk？`, '按来源删除', { type: 'warning' })
    deleting.value = true
    const res = await deleteChunks({ source: sourceFilter.value })
    ElMessage.success(`已删除 ${res.deleted} 个 chunk`)
    sourceFilter.value = ''
    await loadChunks()
    await loadSources()
  } catch (error) {
    if (error !== 'cancel') {
      console.error(error)
      ElMessage.error('删除失败')
    }
  } finally {
    deleting.value = false
  }
}

onMounted(() => {
  loadSources()
  loadChunks()
})
</script>

<style scoped>
.chunk-list-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.page-toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
}

.chunk-text-cell {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.chunk-text {
  flex: 1;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.6;
  font-size: 13px;
  color: #374151;
}

.expand-btn {
  flex-shrink: 0;
  padding: 0;
  margin-top: 2px;
}

.chunk-full-text {
  max-height: 320px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-all;
  font-size: 13px;
  line-height: 1.6;
  color: #374151;
}

.mono-text {
  font-family: 'Cascadia Code', 'Fira Code', Consolas, monospace;
  font-size: 12px;
}
</style>
