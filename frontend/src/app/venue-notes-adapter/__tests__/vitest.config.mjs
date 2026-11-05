import { createRequire } from 'node:module'
const retained = createRequire('/Users/lhs/Desktop/mcr/frontend/package.json')
const names = ['vitest', '@tanstack/react-query', 'react', 'react/jsx-runtime', 'ky', 'zod']
export default {
  root: process.env.NOTES_OVERLAY + '/frontend',
  cacheDir: process.env.NOTES_TMP + '/vite-cache',
  resolve: { alias: names.map(name => ({ find: new RegExp('^' + name + '$'),
    replacement: name === 'vitest' ? retained.resolve(name).replace(/index\.cjs$/, 'dist/index.js') : retained.resolve(name) })) },
  test: { maxWorkers: 1, fileParallelism: false, pool: 'forks',
    include: ['src/app/venue-notes-adapter/__tests__/adapter.test.ts'] },
}
