import { createRoot } from 'react-dom/client'
import { PortsProvider } from '../../../app/access'
import { createProductionPorts } from '../../../app/ports'
import { WorldPanels } from '../../../app/world-panels'
import { MuralWallPanel, useMuralEntry } from '../../../app/mural-entry'
import { inputConfiguration } from '../../../app/world-entry/__tests__/data'
import { createWorldComposition, type WorldComposition } from '../../composition'
import { currentScope, project, resolverFor } from '../../composition/__tests__/fixtures/data'
import { loadPhaserEngine } from '../../scenes/MapScene'
import type { MuralSyncController } from '../../../features/mural/sync'
import { assemblyData } from './assembly-data'
import '../../../shared/styles/base.css'
if (!import.meta.env.DEV) throw Error('Synthetic caller assembly is development-only')
const f = await assemblyData(), omitted = new URLSearchParams(location.search).get('source') === 'omitted'
Object.defineProperty(window, 'WebSocket', { configurable: true, writable: true, value: f.Socket })
await f.login()
let composition: WorldComposition | null = null, sync: MuralSyncController | null = null, lastSync: MuralSyncController | null = null
let created = 0, active = 0, peak = 0, disposed = false
const ports = createProductionPorts(f.auth, { localInput: inputConfiguration, load: async () => {
  composition = createWorldComposition({ auth: f.auth, resolveRelease: resolverFor(f.auth, f.data), ...(omitted ? {} : { mural: f.factory }),
    importEngine: async () => {
      const factory = await loadPhaserEngine()
      return (host, map) => {
        const engine = factory(host, map); created++; active++; peak = Math.max(peak, active)
        void engine.closed.then(() => active--)
        return engine
      }
    } })
  return composition
} })
const scope = f.auth.privacy.capture(), lifetime = new AbortController()
ports.world!.mount(document.getElementById('world')!, { scope: currentScope(f.auth), privacyScope: scope, bootstrap: await project(f.auth, f.data), signal: lifetime.signal })
function Probe() { sync = useMuralEntry().sync; if (sync) lastSync = sync; return null }
const root = createRoot(document.getElementById('root')!)
root.render(<PortsProvider ports={ports}><WorldPanels source={ports.worldAccess!.read()!.presentation!} scope={scope}><Probe /><MuralWallPanel /></WorldPanels></PortsProvider>)
document.getElementById('withdraw')!.onclick = f.revoke
document.getElementById('qualify')!.onclick = () => { f.qualify() }
document.getElementById('policy')!.onclick = () => { f.auth.privacy.changePolicy(scope, 'suspended') }
const probe = {
  read() { return { phase: composition?.replication.read()?.phase, available: !!composition?.presentation?.read()?.mural?.read(),
    revision: sync?.read().revision ?? null, command: lastSync?.editor.read().command?.key ?? null, editor: sync?.editor.read().phase,
    posts: f.requests.filter(r => r.method === 'POST' && r.url.endsWith('/pixels')).map(r => r.headers.get('Idempotency-Key')),
    created, active, peak, sockets: f.sockets.filter(s => s.readyState === 1).length, socketCount: f.sockets.length, ...f.stats() } },
  async inspect() {
    const P = (await import('phaser')).default
    const pool = Reflect.get(P.Display.Canvas.CanvasPool, 'pool') as { parent: unknown; canvas: HTMLCanvasElement }[]
    for (const entry of pool) {
      const game = entry.parent as import('phaser').Game | null
      if (!game || game.canvas !== entry.canvas || !entry.canvas.closest('#world')) continue
      const scene = game.scene.getScenes(true).find(s => s.scene.key === 'mcr-map')
      return { texture: !!scene?.textures.exists('fixture-caller-wall'), objects: scene?.children.list.filter(o => Reflect.get(o, 'texture')?.key === 'fixture-caller-wall').length ?? 0 }
    }
    return { texture: false, objects: 0 }
  },
  lost() { f.server.mode('lost') }, allowResult() { f.server.mode('ack'); f.server.resultAvailable(true) },
  dispose() { if (disposed) return; disposed = true; lifetime.abort(); root.unmount(); ports.dispose?.(); f.dispose() },
}
window.__MURAL_ASSEMBLY__ = probe
declare global { interface Window { __MURAL_ASSEMBLY__: typeof probe } }
