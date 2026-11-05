import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

const temporary = process.env.MCR_PI11_TMP
if (!temporary) throw Error('A private allocated PI11 TMP is required')
export default defineConfig({
  plugins: [react()],
  cacheDir: `${temporary}/vitest-cache`,
  test: {
    setupFiles: ['src/features/world-nearby/__tests__/parade-ink/admission.setup.ts'],
    include: ['src/features/world-nearby/__tests__/parade-ink/nearby.test.tsx', 'src/features/world-rooms/__tests__/world-access.test.ts', 'src/features/world-rooms/__tests__/ui.test.tsx'],
    maxWorkers: 1,
    fileParallelism: false,
  },
})
