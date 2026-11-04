import { defineConfig } from 'vitest/config'
const temporary = process.env.MCR_ORIGINAL_FE34_TMP
if (!temporary?.startsWith('/private/tmp/mcr-original-fe34-')) throw Error('Accepted private original FE34 source allocation required')
export default defineConfig({ cacheDir: `${temporary}/vitest`, test: {
  maxWorkers: 1, fileParallelism: false,
  include: ['src/features/me/__tests__/original-fe34/*.test.tsx', 'src/features/me/__tests__/controller.test.ts', 'src/features/me/__tests__/ui.test.tsx'],
} })
