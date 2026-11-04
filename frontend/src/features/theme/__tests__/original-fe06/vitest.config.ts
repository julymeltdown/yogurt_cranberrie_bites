import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
if (!process.env.MCR_FE06_TEMP) throw new Error('Explicit temporary output prefix required')
export default defineConfig({ plugins: [react()], cacheDir: `${process.env.MCR_FE06_TEMP}/vite`,
  test: { cache: false, maxWorkers: 1, fileParallelism: false } })
