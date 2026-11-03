import { createRoot } from 'react-dom/client'
import { useState, useSyncExternalStore } from 'react'
import type PhaserType from 'phaser'
import { createMuralSync } from '../controller'
import { MuralSyncPanel } from '../MuralSyncPanel'
import { createMuralHttp } from '../http'
import { createMuralTextureSink } from '../../../../world/mural/sink'
import { createPhaserMuralScene } from '../../../../world/mural/phaser'
import { authorityPort, delta, httpClientFixture, ids, pixel, serverFixture, snapshot, timestamp } from './fixture'
import '../../../../shared/styles/base.css'
import './preview.css'

const server = serverFixture(), stage = document.createElement('div')
stage.className = 'sync-fixture-world'; stage.setAttribute('aria-label', 'Two fixture world walls in one Phaser game')
document.body.prepend(stage)
let socketCount = 0
const NativeSocket = window.WebSocket
window.WebSocket = class extends NativeSocket { constructor(url: string | URL, protocols?: string | string[]) { super(url, protocols); socketCount++ } }
const Phaser = (await import('phaser')).default
let ready!: (scene: PhaserType.Scene) => void
const sceneReady = new Promise<PhaserType.Scene>(resolve => { ready = resolve })
class FixtureScene extends Phaser.Scene { create() { ready(this) } }
const game = new Phaser.Game({ type: Phaser.CANVAS, parent: stage, width: 320, height: 152, backgroundColor: '#E5DFD4', scene: FixtureScene,
  render: { pixelArt: true, antialias: false }, audio: { noAudio: true }, banner: false })
const scene = await sceneReady
let serial = 800
const actors = [ids.user, ids.other].map((actor, index) => {
  const authority = authorityPort(), auth = httpClientFixture(server, actor, authority)
  const adapter = createMuralHttp({ http: auth.http, privacy: auth.privacy, authority, now: () => timestamp })
  const sink = createMuralTextureSink(createPhaserMuralScene(scene, { x: index * 160 + 8, y: 8, width: 136, height: 136, depth: 1 }, `fixture-wall-${index}`))
  const sync = createMuralSync({ privacy: auth.privacy, authority, reads: adapter.reads, commands: adapter.commands, sink,
    newKey: () => `00000000-0000-4000-8000-${String(++serial).padStart(12, '0')}` })
  sync.start(); return { auth, authority, sink, sync }
})
function Preview() {
  const [active, setActive] = useState(0)
  const a = useSyncExternalStore(actors[0]!.sync.subscribe, actors[0]!.sync.read), b = useSyncExternalStore(actors[1]!.sync.subscribe, actors[1]!.sync.read)
  return <main className="sync-fixture"><p>FE33 deterministic two-consumer fixture · MOCK_READY · no live collaboration</p>
    <p aria-label="Peer revisions">A revision {a.revision ?? 'unavailable'} · B revision {b.revision ?? 'unavailable'}</p>
    <nav aria-label="Fixture client"><button type="button" aria-pressed={active === 0} onClick={() => setActive(0)}>Client A</button><button type="button" aria-pressed={active === 1} onClick={() => setActive(1)}>Client B</button></nav>
    <MuralSyncPanel key={active} sync={actors[active]!.sync} />
  </main>
}
const controls = {
  read: () => actors.map(a => ({ sync: a.sync.read(), editor: a.sync.editor.read(), resources: a.sink.read() })),
  counts: () => ({ sockets: socketCount, games: 1, placements: server.requests.filter(r => r.method === 'POST').length, requests: server.requests.length }),
  mode: server.mode,
  allowOriginal() { server.resultAvailable(true) },
  async refreshAll() { await Promise.all(actors.map(a => a.sync.refresh())) },
  reordered() {
    const from = server.read().revision, first = pixel(from + 1), second = pixel(from + 2); server.paint(first); server.paint(second)
    const leases = actors.map(a => a.sync.lease()!)
    actors[0]!.sync.receiveDelta(leases[0]!, delta(from, [first, second]))
    actors[1]!.sync.receiveDelta(leases[1]!, delta(from + 1, [second]))
    actors[1]!.sync.receiveDelta(leases[1]!, delta(from, [first])); actors[1]!.sync.receiveDelta(leases[1]!, delta(from, [first, second]))
  },
  async restore(state: 'EDITABLE' | 'PAUSED' | 'FROZEN') {
    const from = server.read().revision, next = snapshot({ revision: from + 20, state, snapshot_id: ids.next })
    server.restore(next)
    actors.forEach(a => a.sync.receiveDelta(a.sync.lease()!, delta(from, [], true, next.revision)))
    await Promise.resolve()
  },
  logout() { actors.forEach(a => a.auth.session.replace(a.auth.session.read().generation, null)) },
  dispose() { actors.forEach(a => { a.sync.dispose(); a.auth.dispose() }); game.destroy(true); window.WebSocket = NativeSocket },
}
declare global { interface Window { __MURAL_SYNC__: typeof controls } }
window.__MURAL_SYNC__ = controls
createRoot(document.getElementById('root')!).render(<Preview />)
