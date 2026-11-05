// Absolute read-only dependency resolution: never create a node_modules link.
import { createRequire } from 'node:module'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
const here = dirname(fileURLToPath(import.meta.url))
export const root = resolve(here, '../../../..')
export const frontend = resolve(root, 'frontend')
export const requireDependency = createRequire(resolve(process.env.MCR_FE37_DEPS_FRONTEND, 'package.json'))
const packages = ['react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom/client', 'react-dom/test-utils',
  'react-dom', 'react', '@testing-library/jest-dom/vitest', '@testing-library/react',
  'react-router-dom', '@tanstack/react-query', 'zod', 'ky', 'phaser', 'browser-image-compression',
  'react-hook-form', '@hookform/resolvers/zod', 'zustand']
export const aliases = packages.map(name => ({ find: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`),
  replacement: name === '@testing-library/jest-dom/vitest'
    ? resolve(dirname(requireDependency.resolve('@testing-library/jest-dom/package.json')), 'dist/vitest.mjs')
    : requireDependency.resolve(name) }))
export default {
  root: frontend, cacheDir: resolve(process.env.MCR_FE37_TMP, 'vitest-cache'),
  resolve: { alias: aliases },
  test: { include: ['src/app/operations-composition/__tests__/member-content-production/*.test.tsx'], maxWorkers: 1, fileParallelism: false },
}
