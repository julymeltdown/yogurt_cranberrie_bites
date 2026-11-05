import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
const temporary = process.env.MCR_VENU_TMP
if (!temporary) throw Error('A private allocated Paradise City temporary directory is required')
export default defineConfig({ plugins: [react()], cacheDir: `${temporary}/vitest`, test: {
  include: ['src/features/paradise-city-map/__tests__/consumer/location-adapter.test.ts', 'src/features/paradise-city-map/__tests__/consumer/location-requests.test.tsx'], maxWorkers: 1, fileParallelism: false,
} })
