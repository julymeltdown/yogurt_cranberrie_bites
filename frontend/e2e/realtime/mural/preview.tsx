import { createRoot } from 'react-dom/client'
import { PortsProvider } from '../../../src/app/access'
import { createProductionPorts } from '../../../src/app/ports'
import { WorldPanels } from '../../../src/app/world-panels'
import { MuralWallPanel, useMuralEntry } from '../../../src/app/mural-entry'
import { inputConfiguration } from '../../../src/app/world-entry/__tests__/data'
import { createWorldComposition, type WorldComposition } from '../../../src/world/composition'
import { currentScope, project, resolverFor } from '../../../src/world/composition/__tests__/fixtures/data'
import { loadPhaserEngine } from '../../../src/world/scenes/MapScene'
import type { MuralSyncController } from '../../../src/features/mural/sync'
import type { MuralReadLease } from '../../../src/features/mural/sync/contract'
import { failureData } from './data'
import '../../../src/shared/styles/base.css'
if (!import.meta.env.DEV) throw Error('Synthetic QA05 fixture is development-only')
const options = new URLSearchParams(location.search), f = await failureData(options.get('client') ?? 'a')
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: f.Socket })
await f.login()
let composition: WorldComposition | null = null, sync: MuralSyncController | null = null, lastSync: MuralSyncController | null = null
let created = 0, active = 0, peak = 0, disposed = false, maxQueue = 0
let saved: { sync: MuralSyncController; lease: MuralReadLease } | null = null
const ports = createProductionPorts(f.auth, { localInput: inputConfiguration, load: async () => {
  composition = createWorldComposition({ auth: f.auth, resolveRelease: resolverFor(f.auth, f.data), ...(options.has('omitted') ? {} : { mural: f.factory }),
    importEngine: async () => { const factory = await loadPhaserEngine(); return (host, map) => {
      const engine = factory(host, map); created++; active++; peak = Math.max(peak, active); void engine.closed.then(() => active--); return engine
    } } })
  return composition
} })
const scope = f.auth.privacy.capture(), lifetime = new AbortController()
ports.world!.mount(document.getElementById('world')!, { scope: currentScope(f.auth), privacyScope: scope, bootstrap: await project(f.auth, f.data), signal: lifetime.signal })
function Probe() { sync = useMuralEntry().sync; if (sync) lastSync = sync; return null }
const root = createRoot(document.getElementById('root')!)
root.render(<PortsProvider ports={ports}><WorldPanels source={ports.worldAccess!.read()!.presentation!} scope={scope}><Probe /><MuralWallPanel /></WorldPanels></PortsProvider>)
const hash = async (bytes: Uint8Array) => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new Uint8Array(bytes)))).map(b => b.toString(16).padStart(2,'0')).join('')
const probe = {
  read() { const editor = lastSync?.editor.read(), world = composition?.replication.read()
    return { phase: world?.phase, visible: document.visibilityState, worldVisible: world?.visible, available: !!composition?.presentation?.read()?.mural?.read(),
      sync: lastSync?.read(), editor: editor?.phase, selection: editor?.selection ?? null, command: editor?.command?.key ?? null,
      confirmed: editor?.snapshot ? { revision: editor.snapshot.revision, state: editor.snapshot.state } : null,
      created, active, peak, maxQueue, socketCount: f.sockets.length, sockets: f.sockets.filter(s => s.readyState === 1).length,
      socketListeners: f.sockets.reduce((n,s) => n + [...s.listeners.values()].reduce((m,v) => m+v.size,0),0), ...f.stats() } },
  async inspect() {
    const P = (await import('phaser')).default
    for (const item of Reflect.get(P.Display.Canvas.CanvasPool,'pool') as { parent: unknown; canvas: HTMLCanvasElement }[]) {
      const game = item.parent as import('phaser').Game | null
      if (!game || game.canvas !== item.canvas || !item.canvas.closest('#world')) continue
      const scene = game.scene.getScenes(true).find(s => s.scene.key === 'mcr-map'), key = 'qa05-synthetic-wall'
      const texture = scene?.textures.exists(key) ?? false
      const canvas = texture ? scene!.textures.get(key).getSourceImage() as HTMLCanvasElement : null
      const pixels = canvas?.getContext('2d')?.getImageData(0,0,128,128).data
      return { texture, objects: scene?.children.list.filter(o => Reflect.get(o,'texture')?.key === key).length ?? 0,
        rgbaHash: pixels ? await hash(new Uint8Array(pixels)) : null }
    }
    return { texture: false, objects: 0, rgbaHash: null }
  },
  async digest() { const snapshot = lastSync?.editor.read().snapshot; return snapshot ? hash(new TextEncoder().encode(JSON.stringify(snapshot.pixels))) : null },
  saveLease() { if (!lastSync?.lease()) throw Error('No current mural lease'); saved = { sync: lastSync, lease: lastSync.lease()! } },
  deliver(packet: unknown, obsolete = false) {
    const owner = obsolete ? saved?.sync : lastSync, lease = obsolete ? saved?.lease : lastSync?.lease()
    if (!owner || !lease) throw Error('No delivery lease')
    const accepted = owner.receiveDelta(lease, packet); maxQueue = Math.max(maxQueue, owner.read().queued)
    return { accepted, state: owner.read() }
  },
  refresh() { void lastSync?.refresh() }, withdraw: f.withdraw, qualify: f.qualify,
  privacy() { f.auth.privacy.changePolicy(scope, 'suspended') },
  dispose() { if (disposed) return; disposed = true; lifetime.abort(); root.unmount(); ports.dispose?.(); f.dispose() },
}
window.__MURAL_FAILURE__ = probe
declare global { interface Window { __MURAL_FAILURE__: typeof probe } }
