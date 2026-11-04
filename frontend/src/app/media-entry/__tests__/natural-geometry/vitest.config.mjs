import { defineConfig, mergeConfig } from 'vitest/config'
import application from '../../../../../vite.config.ts'

if (!process.env.C06_MEDIA_SOURCE_TMP?.startsWith('/private/tmp/mcr-c06-media-source-')) throw new Error('Private allocated source cache required')
export default mergeConfig(application, defineConfig({
  cacheDir: `${process.env.C06_MEDIA_SOURCE_TMP}/vite`,
  test: { fileParallelism: false, maxWorkers: 1 },
}))
