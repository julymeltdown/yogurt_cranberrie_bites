// QA-only fixture entry. No production import, login, service or product diagnostic global.
import { createRoot } from 'react-dom/client'
import { createMuralSync } from '../../src/features/mural/sync/controller'
import { createMuralHttp } from '../../src/features/mural/sync/http'
import { MuralSyncPanel } from '../../src/features/mural/sync/MuralSyncPanel'
import { createMuralTextureSink } from '../../src/world/mural/sink'
import { authorityPort, httpClientFixture, ids, memoryScene, serverFixture, snapshot, testClock, timestamp } from '../../src/features/mural/sync/__tests__/fixture'
import '../../src/shared/styles/base.css'
import './host.css'

const scenario = new URLSearchParams(location.search).get('scenario') ?? 'loaded'
const server = serverFixture(), clock = testClock(), authority = authorityPort(), scene = memoryScene()
server.restore(snapshot({ cooldown_ms: 5000, ...(scenario === 'paused' ? { state: 'PAUSED' as const } : scenario === 'frozen' ? { state: 'FROZEN' as const } : {}) }))
if (scenario === 'read-only') authority.set({ ...authority.read()!, writable: false })
if (scenario === 'unknown') server.mode('lost')
if (scenario === 'pending') server.mode('pending')
const auth = httpClientFixture(server, ids.user, authority)
const adapter = createMuralHttp({ http: auth.http, privacy: auth.privacy, authority, now: () => timestamp + clock.now() })
const sink = createMuralTextureSink(scene.port)
let serial = 600
const sync = createMuralSync({ privacy: auth.privacy, authority, reads: adapter.reads, commands: adapter.commands, clock, sink,
  newKey: () => `00000000-0000-4000-8000-${String(++serial).padStart(12, '0')}` })
sync.start()
const controls = {
  read: () => ({ editor: sync.editor.read(), sync: sync.read(), remaining: sync.editor.remaining() }),
  requests: () => server.requests.map(r => ({ method: r.method, path: new URL(r.url).pathname, query: new URL(r.url).search, key: r.headers.get('Idempotency-Key') })),
  counts: () => ({ placements: server.requests.filter(r => r.method === 'POST').length, reads: server.requests.filter(r => r.method === 'GET').length }),
  refresh: sync.refresh,
  mode: server.mode,
  allowOriginal() { server.mode('ack'); server.resultAvailable(true) },
  advance: clock.advance,
  dispose() { sync.dispose(); auth.dispose() },
}
declare global { interface Window { __WALL_REFERENCE__: typeof controls } }
window.__WALL_REFERENCE__ = controls
createRoot(document.getElementById('root')!).render(<main className="wall-reference-host"><MuralSyncPanel sync={sync} /></main>)
