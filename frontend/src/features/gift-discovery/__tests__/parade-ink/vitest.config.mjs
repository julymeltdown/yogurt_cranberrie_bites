import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
const root = fileURLToPath(new URL('../../../../../', import.meta.url))
const temporary = process.env.MCR_PI04_TEMP
if (!temporary) throw Error('A fresh allocated PI04 temporary prefix is required')
export default defineConfig({ root, plugins: [react()], cacheDir: resolve(temporary, 'unit-cache'), test: {
  environment: 'node', pool: 'threads', fileParallelism: false, maxWorkers: 1,
  include: [
    'src/features/gift-discovery/__tests__/parade-ink/presentation.test.tsx',
    'src/features/giveaway-authoring/__tests__/parade-ink/presentation.test.tsx',
    'src/features/gift-pickup/__tests__/parade-ink/presentation.test.tsx',
    'src/app/community-entry/__tests__/parade-ink-pickup-media/scope.test.ts',
    'src/app/community-entry/__tests__/routes.test.tsx',
    'src/features/gift-pickup/__tests__/pickup.test.tsx',
    'src/features/giveaway-authoring/__tests__/authoring.test.tsx',
    'src/features/gift-discovery/__tests__/client-completion.test.tsx',
  ],
} })
