import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      '/api': 'http://localhost:3000',
    },
  },
  build: {
    // Tailwind v4 requires a modern baseline (Safari 16.4+, Chrome 111+, Firefox 128+).
    target: ['chrome111', 'edge111', 'firefox128', 'safari16.4'],
    sourcemap: false,
  },
})
