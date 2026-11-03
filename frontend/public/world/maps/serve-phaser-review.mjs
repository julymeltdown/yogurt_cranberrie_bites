// Read-only Vite harness. Its cache goes to temporary scratch, never another checkout.
import { createRequire } from 'node:module'
import { dirname, resolve, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { execFileSync } from 'node:child_process'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, '../../../..')
const modules = process.env.MCR_NODE_MODULE_ROOT ?? '/Users/lhs/Desktop/mcr/frontend/node_modules'
const require = createRequire(resolve(modules, '../package.json'))
const { createServer } = await import(pathToFileURL(require.resolve('vite')))
const sourceCheckout = process.env.MCR_AR04_CHECKOUT ?? '/Users/lhs/.codex/worktrees/1ff9/mcr'
const sourceCommit = process.env.MCR_AR04_RESOURCE_COMMIT ?? 'f711c6a4d15e794504026bd39dc77d20c5d21273'
const readCommitted = path => execFileSync('git', ['-C', sourceCheckout, 'show', `${sourceCommit}:frontend/public/world/themes/${path}`])
const inputs = JSON.parse(readCommitted('binding-inputs.json'))
const temporary = await mkdtemp(join(tmpdir(), 'mcr-ar02-vite-'))
const server = await createServer({ configFile: false, root: join(root, 'frontend'), cacheDir: temporary,
  resolve: { alias: { phaser: require.resolve('phaser/dist/phaser.js') } },
  optimizeDeps: { noDiscovery: true, entries: [], include: ['phaser'] },
  server: { host: '127.0.0.1', port: 8874, strictPort: true, fs: { allow: [root, modules] } },
  plugins: [{ name: 'ar02-committed-textures-only', configureServer(vite) {
    vite.middlewares.use((req, res, next) => {
      const path = req.url?.split('?')[0]
      if (path === '/world/maps/phaser-texture-inputs.json') {
        const bytes = JSON.stringify({ sourceCommit, themes: inputs.themes })
        res.setHeader('Content-Type', 'application/json'); res.end(bytes); return
      }
      const name = /^\/world\/themes\/(revenge|parade|danger)\.png$/.exec(path ?? '')?.[1]
      if (!name) { next(); return }
      const bytes = readCommitted(name + '.png')
      res.setHeader('Content-Type', 'image/png'); res.setHeader('Content-Length', bytes.length); res.end(bytes)
    })
  } }] })
const cleanup = async () => { await server.close(); await rm(temporary, { recursive: true, force: true }); process.exit(0) }
process.once('SIGTERM', cleanup); process.once('SIGINT', cleanup)
await server.listen()
console.log(`AR02 actual FE14/Phaser terrain review http://127.0.0.1:8874/world/maps/phaser-review.html; committed textures ${sourceCommit}`)
