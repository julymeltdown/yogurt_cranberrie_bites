import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'node:url'
if (!process.env.MCR_FE06_TEMP) throw new Error('Explicit temporary output prefix required')
export default defineConfig({ root: fileURLToPath(new URL('../../../../../', import.meta.url)), plugins: [react()], cacheDir: `${process.env.MCR_FE06_TEMP}/vite`,
  test: { cache: false, maxWorkers: 1, fileParallelism: false } })
