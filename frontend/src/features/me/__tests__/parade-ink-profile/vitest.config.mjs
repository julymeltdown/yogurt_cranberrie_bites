import { defineConfig } from 'vitest/config'
const temporary = process.env.MCR_PI06_TMP
if (!temporary?.startsWith('/private/tmp/mcr-pi06-')) throw Error('Fresh finite PI06 execution admission required')
export default defineConfig({ cacheDir: `${temporary}/vitest`, test: {
  maxWorkers: 1, fileParallelism: false,
  include: ['src/features/me/__tests__/parade-ink-profile/presentation.test.tsx'],
} })
