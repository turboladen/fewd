import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    // Per-test ceiling. The default 5s is too tight for the fuller component
    // tests (MealPlanner et al. can legitimately cross 10s on a cold CI
    // runner while a cargo build hogs I/O in parallel).
    testTimeout: 15000,
  },
})
