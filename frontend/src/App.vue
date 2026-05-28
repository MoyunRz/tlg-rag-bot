<script setup lang="ts">
import {
  DataBoard,
  Grid,
  Monitor,
  Notebook,
} from '@element-plus/icons-vue'
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

const route = useRoute()
const router = useRouter()

const menuItems = [
  { index: '/dashboard', label: '概览', icon: DataBoard },
  { index: '/knowledge-base', label: '知识库', icon: Notebook },
  { index: '/chunks', label: 'Chunk 列表', icon: Grid },
  { index: '/debug', label: '在线调试', icon: Monitor },
]

const activeMenu = computed(() => route.path)

const handleSelect = (index: string) => {
  router.push(index)
}
</script>

<template>
  <el-container class="layout-shell">
    <el-aside class="layout-aside" width="220px">
      <div class="brand-block">
        <div class="brand-icon">
          <svg viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
            <rect width="32" height="32" rx="8" fill="url(#g)" />
            <path d="M9 12h14M9 16h10M9 20h12" stroke="#fff" stroke-width="2" stroke-linecap="round" />
            <circle cx="24" cy="22" r="4" fill="#34d399" stroke="#111827" stroke-width="1.5" />
            <defs><linearGradient id="g" x1="0" y1="0" x2="32" y2="32"><stop stop-color="#6366f1" /><stop offset="1" stop-color="#8b5cf6" /></linearGradient></defs>
          </svg>
        </div>
        <div class="brand-text">
          <h1>tlg-rag-bot</h1>
          <p>Telegram RAG 客服</p>
        </div>
      </div>

      <nav class="nav-menu">
        <a
          v-for="item in menuItems"
          :key="item.index"
          class="nav-item"
          :class="{ active: activeMenu === item.index }"
          @click="handleSelect(item.index)"
        >
          <el-icon :size="18"><component :is="item.icon" /></el-icon>
          <span>{{ item.label }}</span>
        </a>
      </nav>

      <div class="aside-footer">
        <div class="status-dot"></div>
        <span>服务运行中</span>
      </div>
    </el-aside>

    <el-container>
      <el-header class="layout-header">
        <div>
          <div class="page-title">{{ route.meta.title ?? '后台' }}</div>
          <div class="page-subtitle">Rust + Axum + Teloxide + Chroma + Vue 3</div>
        </div>
        <el-tag type="success">MVP Scaffold</el-tag>
      </el-header>

      <el-main class="layout-main">
        <router-view />
      </el-main>
    </el-container>
  </el-container>
</template>

<style scoped>
.layout-aside {
  background: #111827;
  color: #fff;
  display: flex;
  flex-direction: column;
  padding: 0;
  overflow: hidden;
}

.brand-block {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 20px 16px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.brand-icon svg {
  width: 36px;
  height: 36px;
  flex-shrink: 0;
}

.brand-text h1 {
  font-size: 16px;
  font-weight: 700;
  margin: 0;
  color: #fff;
  letter-spacing: -0.02em;
}

.brand-text p {
  margin: 2px 0 0;
  font-size: 12px;
  color: #6b7280;
}

.nav-menu {
  flex: 1;
  padding: 12px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 8px;
  color: #9ca3af;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s ease;
  text-decoration: none;
  user-select: none;
}

.nav-item:hover {
  color: #e5e7eb;
  background: rgba(255, 255, 255, 0.06);
}

.nav-item.active {
  color: #fff;
  background: rgba(99, 102, 241, 0.15);
}

.nav-item.active .el-icon {
  color: #818cf8;
}

.aside-footer {
  padding: 16px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: #6b7280;
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #34d399;
  box-shadow: 0 0 6px rgba(52, 211, 153, 0.5);
}
</style>
