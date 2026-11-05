export default {
  cacheDir: `${process.env.PI08_SCRATCH}/vite-cache`,
  test: { include: ['src/features/parade-ink/world-hud/__tests__/finish-01/photo2-mural-projection.test.ts'],
    environment: 'node', pool: 'forks', maxWorkers: 1, minWorkers: 1, fileParallelism: false },
}
