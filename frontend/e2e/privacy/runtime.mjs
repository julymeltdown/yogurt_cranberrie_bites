import { createRequire } from 'node:module'
import { dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
export const here = dirname(fileURLToPath(import.meta.url))
export const root = resolve(here, '../../..')
// Existing retained installation only. No install, copy or central node_modules link.
export const dependencyRoot = resolve(process.env.MCR_QA04_DEPENDENCIES ?? '/Users/lhs/Desktop/mcr/frontend')
export const requireDependency = createRequire(resolve(dependencyRoot, 'package.json'))
export async function createHarnessServer() {
  const { createServer } = await import(pathToFileURL(requireDependency.resolve('vite')).href)
  return createServer({ configFile: false, root: here, cacheDir: resolve(here, '.cache'), logLevel: 'error',
    resolve: { dedupe: [...Object.keys(requireDependency('./package.json').dependencies), '@tanstack/query-core'] },
    ssr: { noExternal: true }, optimizeDeps: { noDiscovery: true, include: ['react', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom', '@tanstack/react-query', 'ky', 'zod'] },
    server: { host: '127.0.0.1', port: 0, fs: { allow: [root, dependencyRoot] } } })
}
