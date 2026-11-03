import { useLayoutEffect, useState, type ReactNode } from 'react'
import { createRoot } from 'react-dom/client'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import type Phaser from 'phaser'
import type { ReplicationView } from '../../../world/network/replication'
import { createPhaserMuralScene, createWorldMuralAuthority } from '../../../world/mural'
import type { MuralSyncController } from '../../../features/mural/sync'
import { createAppRoutes } from '../../router'
import { bootstrapFor, createFixturePorts } from '../../__tests__/fixtures/ports'
import { MuralEntryProvider, MuralWallPanel, useMuralEntry } from '../provider'
import type { MuralEntryRelease } from '../ports'
import { entryFixture, ids } from './fixtures'
import '../../../shared/styles/base.css'
import '../../shell.css'

if (!import.meta.env.DEV) throw new Error('Synthetic FE33 application fixture is development-only')
const f = entryFixture(), scope = f.scope, mode = new URLSearchParams(location.search).get('state') ?? 'loaded'
const world = createFixturePorts(), listeners = new Set<() => void>()
let scene: Phaser.Scene | null = null, game: Phaser.Game | null = null, sync: MuralSyncController | null = null
let release: MuralEntryRelease | null = null, signal = new AbortController(), setRelease: (value: MuralEntryRelease | null) => void = () => {}
let disposed = false, gamesCreated = 0, gamesActive = 0, peakGames = 0
const binding = { eventId: ids.event, roomId: ids.room, roomEpoch: ids.epoch, streamId: ids.stream }
let view = { scope, phase: 'live', visible: true, baseline: binding } as unknown as ReplicationView
const authority = createWorldMuralAuthority({ privacy: f.auth.privacy,
  world: { read: () => view, subscribe(fn) { listeners.add(fn); return () => { listeners.delete(fn) } } }, writable: () => mode !== 'read-only' })
function publishRelease(incumbent?: Phaser.Scene) {
  scene = incumbent ?? scene
  if (!scene || disposed) return
  signal = new AbortController()
  const port = createPhaserMuralScene(scene, { x: 16, y: 16, width: 128, height: 128, depth: 1 }, 'synthetic-fe33-wall')
  if (!port.onClose) throw new Error('Missing incumbent scene close port')
  release = { scope, binding, signal: signal.signal, authority, scene: { ...port, onClose: port.onClose } }
  setRelease(mode === 'unavailable' ? null : release)
}
const ports = { ...world.ports, auth: f.auth, session: f.auth.session,
  bootstrap: { load: async (scope: Parameters<typeof bootstrapFor>[0]) => bootstrapFor(scope) },
  world: { mount(host: HTMLElement) {
    let stopped = false
    const Phaser = import('phaser')
    void Phaser.then(({ default: P }) => {
      if (stopped || disposed) return
      game = new P.Game({ type: P.CANVAS, width: 160, height: 160, parent: host, backgroundColor: '#201D1D',
        audio: { noAudio: true }, banner: false, render: { pixelArt: true }, scene: { create(this: Phaser.Scene) { publishRelease(this) } } })
      gamesCreated++; gamesActive++; peakGames = Math.max(peakGames, gamesActive)
    })
    return { dispose() { if (stopped) return; stopped = true; signal.abort(); scene = null; if (game) { game.destroy(true); game = null; gamesActive-- } },
      setInputBlocked() {}, setVisible() {} }
  } },
}
const router = createMemoryRouter(createAppRoutes(ports, { panels: { '/wall': <MuralWallPanel />, '/feed': <p>Other synthetic panel</p> } }), { initialEntries: ['/wall'] })
function Probe() { sync = useMuralEntry().sync; return null }
function Fixture({ children }: { children: ReactNode }) {
  const [current, setCurrent] = useState<MuralEntryRelease | null>(null)
  useLayoutEffect(() => { setRelease = setCurrent; if (release && mode !== 'unavailable') setCurrent(release); return () => { setRelease = () => {} } }, [])
  return <><p className="synthetic-entry-label">SYNTHETIC FE33 app route · actual incumbent Phaser/sink + canonical Auth/HTTP. In-process C04 server/authority and geometric raster; no live socket, product art or two-user proof.</p>
    <style>{`.synthetic-entry-label{position:sticky;top:0;z-index:10000;background:#eee6d5;color:#201d1d;font:11px/1.3 system-ui;padding:4px;margin:0}`}</style>
    <MuralEntryProvider auth={f.auth} scope={scope} release={current}><Probe />{children}</MuralEntryProvider></>
}
const root = createRoot(document.getElementById('root')!)
root.render(<Fixture><RouterProvider router={router} /></Fixture>)
window.__MURAL_ENTRY__ = {
  state: () => ({ mode, path: router.state.location.pathname, sync: sync?.read() ?? null, editor: sync?.editor.read().phase ?? null,
    commandHeld: !!sync?.editor.read().command, gamesCreated, gamesActive, peakGames, authorityListeners: listeners.size,
    texturePresent: !!scene?.textures.exists('synthetic-fe33-wall'), posts: f.requests.filter(r => r.method === 'POST').length }),
  lossMode() { f.server.mode('lost') },
  navigate: (path: string) => router.navigate(path),
  disconnect() { signal.abort() }, reconnect: publishRelease,
  policy() { f.auth.privacy.changePolicy(scope, 'block') },
  epoch() { view = { ...view, baseline: { ...view.baseline!, roomEpoch: ids.next } }; for (const fn of Array.from(listeners)) fn() },
  dispose() { if (disposed) return; disposed = true; signal.abort(); root.unmount(); router.dispose(); f.dispose() },
}
declare global { interface Window { __MURAL_ENTRY__: {
  state(): { mode: string; path: string; sync: ReturnType<MuralSyncController['read']> | null; editor: string | null; commandHeld: boolean; gamesCreated: number; gamesActive: number; peakGames: number; authorityListeners: number; texturePresent: boolean; posts: number }
  lossMode(): void; navigate(path: string): Promise<void>; disconnect(): void; reconnect(): void; policy(): void; epoch(): void; dispose(): void
} } }
