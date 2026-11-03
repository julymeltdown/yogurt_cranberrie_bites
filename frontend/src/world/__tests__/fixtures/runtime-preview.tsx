import { StrictMode, useLayoutEffect, useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { createWorldRuntime, type WorldRuntime } from '../../runtime'
import { createAssetResources, loadMap } from '../../map/resources'
import { loadPhaserEngine } from '../../scenes/MapScene'
import { synthetic } from './synthetic'
const fixture = await synthetic()
let current: WorldRuntime | null = null, mode = 'valid', allocations = 0
const resources = createAssetResources({ origin: location.origin, allow: fixture.allow })
function Surface() {
  const host = useRef<HTMLDivElement>(null), [status, setStatus] = useState('stopped')
  useLayoutEffect(() => {
    const runtime = createWorldRuntime({ host: host.current!, timeoutMs: 1500,
      load: async signal => {
        const map = structuredClone(fixture.map)
        if (mode === 'invalid') map.geometry.spawns[0]!.cell_x = 0
        return loadMap({ mapText: JSON.stringify(map), themeText: JSON.stringify(fixture.theme), pin: fixture.pin, resources }, signal)
      },
      importEngine: async () => {
        if (mode === 'slow-import') await new Promise(resolve => setTimeout(resolve, 500))
        const factory = await loadPhaserEngine()
        return (host, map) => {
          allocations++
          const engine = factory(host, map)
          if (mode === 'stop-boot') queueMicrotask(() => current?.stop())
          return engine
        }
      },
    })
    current = runtime
    const unsubscribe = runtime.subscribe(() => { const state = runtime.read(); setStatus(state.phase + ('reason' in state ? `: ${state.reason}` : '')) })
    return () => { unsubscribe(); runtime.dispose(); if (current === runtime) current = null }
  }, [])
  return <><p role="status" aria-live="polite">{status}</p><div ref={host} className="map-host" aria-label="Synthetic map canvas" /></>
}
function App() {
  const [mounted, mount] = useState(true)
  return <main><h1>Map runtime test</h1><p>SYNTHETIC FE14 · deterministic 5×5 grid and three terrain swatches. No character art, venue, movement, multiplayer or world admission.</p>
    <div className="controls"><button onClick={() => void current?.start()}>Start runtime</button><button onClick={() => current?.stop()}>Stop runtime</button>
      <button onClick={() => mount(value => !value)}>{mounted ? 'Unmount runtime' : 'Mount runtime'}</button></div>
    {mounted && <Surface />}
    <p>The virtual default foot position is (48, 48) map pixels. Gold overdraw sits above terrain. No physical location is represented.</p>
    <style>{`*{box-sizing:border-box}body{margin:0;background:#f2eee5;color:#231e1b;font:16px/1.5 system-ui}main{max-width:60rem;margin:auto;padding:20px}h1{font-size:2rem;margin:0 0 12px}p{overflow-wrap:anywhere}.controls{display:flex;gap:8px;flex-wrap:wrap}button{min-width:44px;min-height:44px;padding:8px 12px;border:1px solid #655b50;border-radius:4px;color:#231e1b;background:#fffaf0;font:inherit;cursor:pointer}button:focus-visible{outline:3px solid #873342;outline-offset:3px}.map-host{width:100%;height:55vh;min-height:140px;max-height:600px;background:#151019;margin-top:12px;overflow:hidden}.map-host canvas{display:block;image-rendering:pixelated}`}</style>
  </main>
}
declare global { interface Window { __FE14__: {
  mode(value: string): void; start(): Promise<unknown> | undefined; stop(): void; state(): unknown
  visible(value: boolean): void; allocations(): number; settled(): Promise<void> | undefined
} } }
window.__FE14__ = { mode(value) { mode = value }, start: () => current?.start(), stop: () => current?.stop(), state: () => current?.read(),
  visible: value => current?.setVisible(value), allocations: () => allocations, settled: () => current?.settled() }
createRoot(document.getElementById('root')!).render(<StrictMode><App /></StrictMode>)
