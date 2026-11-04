import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

// Borrowed dependency tree stays immutable. Configuration uses the runner loader;
// all optional output is routed to the explicit resource-owned temporary prefix.
if (!process.env.MCR_FE34_RUNTIME_TEMP) throw new Error('Explicit temporary output prefix required')
export default defineConfig({
  plugins: [react()],
  cacheDir: `${process.env.MCR_FE34_RUNTIME_TEMP}/vite-cache`,
  test: { cache: false, maxWorkers: 1, fileParallelism: false },
})
