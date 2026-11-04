import { defineConfig } from 'vitest/config'
const temporary = process.env.MCR_PROFILE_MEDIA_TMP
if (!temporary?.startsWith('/private/tmp/mcr-profile-activity-media-')) throw Error('Accepted private Profile activity allocation required')
export default defineConfig({ cacheDir: `${temporary}/vitest`, test: {
  maxWorkers: 1, fileParallelism: false,
  include: ['src/features/me/__tests__/profile-activity-media/*.test.tsx', 'src/features/me/__tests__/reference-target/presentation.test.tsx', 'src/features/profile/__tests__/reference-target/presentation.test.tsx'],
} })
