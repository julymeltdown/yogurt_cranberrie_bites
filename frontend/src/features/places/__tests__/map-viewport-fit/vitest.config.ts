import { defineConfig } from 'vitest/config'

const temporary = process.env.MCR_MAP_FIT_TMP
if (!temporary) throw Error('A private allocated Map fit TMP is required')
export default defineConfig({
  cacheDir: `${temporary}/vitest-cache`,
  test: {
    include: ['src/features/places/__tests__/map-viewport-fit/framing.test.ts',
      'src/features/places/__tests__/reference-map/portrait.test.ts'],
    maxWorkers: 1,
    fileParallelism: false,
  },
})
