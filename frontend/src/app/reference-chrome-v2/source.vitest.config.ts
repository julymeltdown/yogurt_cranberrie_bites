import { defineConfig, mergeConfig } from 'vitest/config'
import base from '../../../vite.config'

const cacheDir = process.env.MCR_CHROME_CACHE
if (!cacheDir?.startsWith('/private/tmp/mcr-reference-chrome-v2-')) throw new Error('Private admitted source cache required')
export default mergeConfig(base, defineConfig({ cacheDir, test: { maxWorkers: 1, fileParallelism: false } }))
