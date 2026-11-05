if (!process.env.MCR_PI07_CACHE) throw Error('A booked PI07 cache directory is required')
export default {
  cacheDir: process.env.MCR_PI07_CACHE,
  test: {
    include: ['src/features/avatar/__tests__/parade-ink/static-*.test.tsx'],
    maxWorkers: 1, fileParallelism: false,
  },
}
