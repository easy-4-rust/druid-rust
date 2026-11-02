import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// 前端与后端完全解耦：开发期通过代理访问本地 Admin，
// 构建期产出纯静态资源，由 Admin 内嵌分发。
export default defineConfig({
  plugins: [vue()],
  base: './',
  build: {
    outDir: process.env.DRUID_CONSOLE_DIST || 'dist',
    emptyOutDir: true,
    assetsDir: 'assets',
  },
  server: {
    proxy: {
      '/druid': {
        target: process.env.DRUID_ADMIN_URL || 'http://127.0.0.1:8080',
        changeOrigin: true,
      },
    },
  },
})
