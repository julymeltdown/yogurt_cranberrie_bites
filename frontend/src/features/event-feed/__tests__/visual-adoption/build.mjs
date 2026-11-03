import { build, preview } from 'vite'
import { resolve } from 'node:path'
const frontend = resolve(import.meta.dirname, '../../../../..')
export const outDir = '/tmp/mcr-uiv03-community-build'
export async function bundle() {
  await build({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), logLevel: 'warn', build: { outDir, emptyOutDir: true,
    rollupOptions: { input: [resolve(frontend, 'index.html'), resolve(frontend, 'src/app/__tests__/fixtures/community.html'), resolve(import.meta.dirname, 'preview.html'),
      resolve(frontend, 'src/features/party-discovery/__tests__/fixtures/preview.html'), resolve(frontend, 'src/features/gift-discovery/__tests__/fixtures/preview.html')] } } })
}
export async function serve() { return preview({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), build: { outDir }, preview: { host: '127.0.0.1', port: 0, strictPort: false } }) }
if (process.argv[1] === import.meta.filename) await bundle()
