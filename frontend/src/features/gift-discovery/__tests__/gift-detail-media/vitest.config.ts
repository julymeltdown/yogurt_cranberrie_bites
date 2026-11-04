import { defineConfig } from 'vitest/config'
import { env } from 'node:process'
import { resolve } from 'node:path'
import { adoptionPlugin } from './adoption-inputs'
const temporary = env.MCR_GIFTS_MEDIA_TEMP
if (!temporary) throw Error('An allocated owned temporary prefix is required')
export default defineConfig({ plugins: [adoptionPlugin()], cacheDir: resolve(temporary, 'vite-cache'), test: { environment: 'node',
  include: ['src/features/gift-discovery/__tests__/**/*.test.{ts,tsx}', 'src/app/media-entry/__tests__/root-binding/provider.test.tsx'],
  fileParallelism: false, maxWorkers: 1 } })
