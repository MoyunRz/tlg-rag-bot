import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  server: {
    proxy: {
      '/api': {
        target: 'http://149.104.28.237:4000',
        changeOrigin: true,
      },
      '/telegram': {
        target: 'http://149.104.28.237:4000',
        changeOrigin: true,
      },
    },
  },
})
