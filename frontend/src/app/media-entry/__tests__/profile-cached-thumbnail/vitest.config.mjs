import { defineConfig } from 'vitest/config'
const temporary = process.env.MCR_PROFILE_MEDIA_TMP
if (!temporary?.startsWith('/private/tmp/mcr-profile-activity-media-')) throw Error('Accepted private Profile media allocation required')
export default defineConfig({ cacheDir: `${temporary}/cached-vitest`, test: {
  maxWorkers: 1, fileParallelism: false,
  include: ['src/app/media-entry/__tests__/profile-cached-thumbnail/*.test.*', 'src/app/media-entry/__tests__/owner.test.ts'],
  environmentOptions: { jsdom: { url: 'https://mcr.example' } },
} })
