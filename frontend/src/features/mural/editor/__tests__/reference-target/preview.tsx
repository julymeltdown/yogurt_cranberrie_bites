import { createRoot } from 'react-dom/client'
import { createMuralSync } from '../../../sync/controller'
import { MuralSyncPanel } from '../../../sync/MuralSyncPanel'
import { createMuralTextureSink } from '../../../../../world/mural/sink'
import { authorityPort, delta, httpClientFixture, memoryScene, pixel, serverFixture, snapshot, testClock } from '../../../sync/__tests__/fixture'
import './preview.css'

const server = serverFixture(), authority = authorityPort(), client = httpClientFixture(server, undefined, authority), clock = testClock(), scene = memoryScene()
// Original deterministic pixel fixture, not a screenshot, reference-image texture or product data.
const pixels = Array.from({ length: 16384 }, (_, n) => {
  const x = n % 128, y = Math.floor(n / 128), stripe = x < 12 + Math.floor(y / 10) || x > 115 - Math.floor(y / 10)
  return stripe ? 2 : (x > 47 && x < 79 && y > 20 && y < 109 && (Math.floor(x / 8) + Math.floor(y / 10)) % 3 === 0) ? 0 : 1
})
server.restore(snapshot({ pixels }))
const http = client.adapter(true), sink = createMuralTextureSink(scene.port)
const sync = createMuralSync({ privacy: client.privacy, authority: client.authority, clock, sink, reads: http.reads, commands: http.commands })
const hooks = {
  read: () => ({ editor: sync.editor.read(), sync: sync.read() }),
  counts: () => ({ posts: server.requests.filter(r => r.method === 'POST').length, gets: server.requests.filter(r => r.method === 'GET').length }),
  mode: (value: 'ack' | 'lost' | 'pending' | 'rejected') => server.mode(value),
  async restore(state: 'EDITABLE' | 'PAUSED' | 'FROZEN') { server.restore(snapshot({ pixels, revision: server.read().revision + 20, state })); await sync.refresh() },
  async cooldown() { server.restore({ ...server.read(), revision: server.read().revision + 1, next_allowed_at: '2026-11-04T00:01:00Z' }); await sync.refresh() },
  readonly() { const old = client.authority.read(); if (old) authority.set({ ...old, writable: false }) },
  gap() { const old = sync.read().revision ?? 10; sync.receiveDelta(sync.lease()!, delta(old + 1, [pixel(old + 2)])) },
  logout() { client.session.replace(client.session.read().generation, null) },
  refresh: () => sync.refresh(),
  dispose() { sync.dispose(); client.dispose() },
}
declare global { interface Window { __WALL_REFERENCE__: typeof hooks } }
window.__WALL_REFERENCE__ = hooks
sync.start()
createRoot(document.getElementById('root')!).render(<main><MuralSyncPanel sync={sync} /></main>)
