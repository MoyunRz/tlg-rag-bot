import { createRouter, createWebHistory } from 'vue-router'

import ChunkListView from '../views/ChunkListView.vue'
import DashboardView from '../views/DashboardView.vue'
import DebugView from '../views/DebugView.vue'
import KnowledgeBaseView from '../views/KnowledgeBaseView.vue'

export const routes = [
  { path: '/', redirect: '/dashboard' },
  { path: '/dashboard', name: 'dashboard', component: DashboardView, meta: { title: '概览' } },
  { path: '/knowledge-base', name: 'knowledge-base', component: KnowledgeBaseView, meta: { title: '知识库' } },
  { path: '/chunks', name: 'chunks', component: ChunkListView, meta: { title: 'Chunk 列表' } },
  { path: '/debug', name: 'debug', component: DebugView, meta: { title: '在线调试' } },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

router.afterEach((to) => {
  document.title = `tlg-rag-bot · ${String(to.meta.title ?? '后台')}`
})

export default router
