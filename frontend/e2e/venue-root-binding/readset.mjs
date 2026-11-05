// Read immutable inputs into memory. No checkout, source copy, dependency link or install.
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { dirname, resolve } from 'node:path'
export const frontend = resolve(import.meta.dirname, '../..')
export const root = resolve(frontend, '..')
const read = (pin, path) => execFileSync('git', ['show', `${pin}:${path}`], { cwd: root, maxBuffer: 2 * 1024 * 1024 })
const map = '1c5c991676844402e694037bd95aa72e6cf7e3da'
const spatial = 'd3dd189c', community = '7acd0ec240bae068f1714ac5f20477dde0a6425e'
const inputs = [
  ...['ParadiseCityMapPanel.tsx', 'consumer.ts', 'capability-adapters.ts', 'browser-location.ts'].map(name => [map, `frontend/src/app/venue-entry/${name}`]),
  ...['index.ts', 'model/controller.ts', 'model/siteplan-view.ts', 'ui/MapCanvas.tsx', 'ui/ParadiseCityMap.tsx', 'ui/paradise-city-map.css'].map(name => [map, `frontend/src/features/paradise-city-map/${name}`]),
  ['e9535129', 'frontend/src/features/paradise-city-map/ports.ts'],
  ...['contract.ts', 'image-resource.ts', 'model.ts', 'registration.ts', 'viewport.ts'].map(name => [spatial, `frontend/src/features/paradise-city-map/spatial/${name}`]),
  ...['index.ts', 'owner.ts', 'ports.ts', 'provider.tsx', 'routes.tsx', 'view.tsx', 'adapter.ts'].map(name => [community, `frontend/src/app/venue-community-entry/${name}`]),
  ...['Invitation.tsx', 'model.ts', 'meeting.css'].map(name => [community, `frontend/src/features/venue-invitations/${name}`]),
  [community, 'frontend/src/features/meeting-conversation/Conversation.tsx'],
  ['81680fc8995246c2e03e1a8f5fc1221a0785a535', 'frontend/src/shared/api/paradise-city-v1/meeting.ts'],
]
export const overlay = new Map(), pins = []
for (const [source, path] of inputs) {
  const bytes = read(source, path), absolute = resolve(root, path)
  overlay.set(absolute, bytes.toString('utf8'))
  pins.push({ source: execFileSync('git', ['rev-parse', source], { cwd: root, encoding: 'utf8' }).trim(), path,
    bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') })
}
const artPaths = ['asset-interface.json', 'siteplan-photo-5.jpg']
export const art = new Map(artPaths.map(name => [name, read(spatial, `frontend/public/venue/paradise-city/v1/${name}`)]))
for (const [name, bytes] of art) pins.push({ source: execFileSync('git', ['rev-parse', spatial], { cwd: root, encoding: 'utf8' }).trim(),
  path: `frontend/public/venue/paradise-city/v1/${name}`, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') })
const extensions = ['', '.ts', '.tsx', '.json', '/index.ts', '/index.tsx']
export function immutableInputs() {
  return { name: 'venue-root-immutable-inputs', enforce: 'pre',
    resolveId(id, importer) {
      if (id === 'virtual:venue-root-art') return '\0venue-root-art'
      const clean = id.split('?')[0]
      const path = clean.startsWith('.') && importer ? resolve(dirname(importer.split('?')[0]), clean) : clean
      for (const extension of extensions) if (overlay.has(path + extension)) return path + extension
    },
    load(id) {
      if (id === '\0venue-root-art') return `export const manifestText=${JSON.stringify(art.get('asset-interface.json').toString('utf8'))};export const imageBase64=${JSON.stringify(art.get('siteplan-photo-5.jpg').toString('base64'))};`
      return overlay.get(id.split('?')[0])
    },
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const bytes = art.get(req.url?.replace('/venue/paradise-city/v1/', ''))
        if (!bytes) return next()
        res.setHeader('Content-Type', req.url.endsWith('.json') ? 'application/json' : 'image/jpeg'); res.end(bytes)
      })
    },
  }
}
