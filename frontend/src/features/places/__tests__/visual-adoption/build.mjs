import { build, preview } from 'vite'
import { resolve } from 'node:path'
import { readFile } from 'node:fs/promises'
const frontend = resolve(import.meta.dirname, '../../../../..')
export const fixturePath = '/src/features/places/__tests__/visual-adoption/preview.html'
export const outDir = '/tmp/mcr-uiv05-map-build'
export async function bundle() {
  await build({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), logLevel: 'warn', build: { outDir, emptyOutDir: true,
    rollupOptions: { input: [resolve(frontend, 'index.html'), resolve(frontend, `.${fixturePath}`)] } } })
}
export async function serve() {
  return preview({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), build: { outDir },
    plugins: [{ name: 'uiv05-document-recovery-fixture', configurePreviewServer(server) {
      server.middlewares.use(async (request, response, next) => {
        if (new URL(request.url, 'http://127.0.0.1').pathname !== '/world/map') return next()
        try { response.setHeader('Content-Type', 'text/html'); response.end(await readFile(resolve(outDir, `.${fixturePath}`), 'utf8')) }
        catch (error) { next(error) }
      })
    } }], preview: { host: '127.0.0.1', port: 0, strictPort: false } })
}
if (process.argv[1] === import.meta.filename) await bundle()
