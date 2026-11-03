import type { WorldCamera } from './camera/port'
import { MapError } from './map/json'
import { limits } from './map/manifest'
import { abortable, type LoadedMap } from './map/resources'
export type RuntimeState = Readonly<{ phase: 'stopped' | 'loading' | 'ready' | 'unavailable' | 'error' | 'disposed';
  reason?: 'invalid' | 'load-failed' | 'timeout' | 'engine-failed' | 'host-busy'; widthPx?: number; heightPx?: number; spawnFoot?: Readonly<{ x: number; y: number }> }>
export interface EngineLease {
  ready: Promise<void>; closed: Promise<void>
  resize(width: number, height: number): void
  setVisible(visible: boolean): void
  destroy(): void
  readonly camera?: WorldCamera
}
export interface ControlAttachment { setEnabled(value: boolean): void; dispose(): void }
export interface ControlContext { readonly host: HTMLElement; readonly map: LoadedMap; readonly camera: WorldCamera; readonly signal: AbortSignal }
export type AttachControls = (context: ControlContext) => ControlAttachment
export type EngineFactory = (host: HTMLElement, map: LoadedMap) => EngineLease
export interface RuntimeOptions {
  host: HTMLElement
  load(signal: AbortSignal): Promise<LoadedMap>
  importEngine?: () => Promise<EngineFactory>
  timeoutMs?: number
  attachControls?: AttachControls
}
const claimedHosts = new WeakMap<HTMLElement, { owner: object }>()
/** Explicitly started resource owner. No auth, network admission, avatar or production shell wiring. */
export function createWorldRuntime(options: RuntimeOptions) {
  let state: RuntimeState = Object.freeze({ phase: 'stopped' }), generation = 0, disposed = false, visible = true, inputBlocked = false, visibilityRevision = 0
  let active: { controller: AbortController; mount: HTMLDivElement; engine: EngineLease | null; map: LoadedMap | null; controls: ControlAttachment | null;
    observer: ResizeObserver | null; resize: (() => void) | null; frame: number | null; detach: () => void; claim: { owner: object }; creating: boolean; finish: () => void } | null = null
  let flight: Promise<RuntimeState> | null = null, drain = Promise.resolve()
  const owner = {}, listeners = new Set<() => void>()
  const publish = (next: RuntimeState) => { state = Object.freeze(next); for (const listener of Array.from(listeners)) { try { listener() } catch { /* Observers cannot strand owned resources. */ } } }
  const importEngine = options.importEngine ?? (() => import('./scenes/MapScene').then(m => m.loadPhaserEngine()))
  function stop() {
    const old = active
    if (!old) return
    ++generation; active = null; flight = null; old.controller.abort(); old.detach()
    old.observer?.disconnect()
    if (old.resize) window.removeEventListener('resize', old.resize)
    if (old.frame !== null) cancelAnimationFrame(old.frame)
    old.mount.remove()
    old.finish()
    if (!disposed) publish({ phase: 'stopped' })
  }
  function start(signal?: AbortSignal): Promise<RuntimeState> {
    if (disposed || signal?.aborted) return Promise.resolve(state)
    if (flight) return flight
    const holder = claimedHosts.get(options.host)
    if (holder && holder.owner !== owner) { publish({ phase: 'error', reason: 'host-busy' }); return Promise.resolve(state) }
    const claim = { owner }; claimedHosts.set(options.host, claim)
    const controller = new AbortController(), mount = document.createElement('div'), ticket = ++generation
    mount.style.cssText = 'width:100%;height:100%;overflow:hidden'
    const abort = () => stop()
    const owned = { controller, mount, engine: null as EngineLease | null, map: null as LoadedMap | null,
      controls: null as ControlAttachment | null, observer: null as ResizeObserver | null, resize: null as (() => void) | null, frame: null as number | null,
      detach: () => signal?.removeEventListener('abort', abort), claim, creating: false, finish: () => {} }
    let finished = false
    owned.finish = () => {
      if (finished || owned.creating) return
      finished = true
      try { owned.controls?.dispose() } catch { /* A broken attachment must not strand engine teardown. */ }
      owned.controls = null
      const release = () => { try { owned.map?.dispose() } finally { owned.map = null; if (claimedHosts.get(options.host) === claim) claimedHosts.delete(options.host) } }
      if (owned.engine) {
        try { owned.engine.destroy() } catch { /* Continue releasing ownership after a broken injected engine. */ }
        drain = Promise.all([drain, owned.engine.closed]).then(release, release)
      } else drain = drain.then(release, release)
    }
    active = owned
    signal?.addEventListener('abort', abort, { once: true })
    const current = () => !disposed && active === owned && generation === ticket && !controller.signal.aborted
    const task = Promise.resolve().then(async () => {
      let stage: 'load' | 'engine' = 'load'
      try {
        await abortable(drain, controller.signal, options.timeoutMs ?? limits.deadlineMs)
        if (!current()) return state
        const load = options.load(controller.signal).then(map => {
          // Reserve the image before resolving the await boundary; cancellation can run between microtasks.
          if (current()) owned.map = map
          else map.dispose()
          return map
        })
        const map = await abortable(load, controller.signal, options.timeoutMs ?? limits.deadlineMs)
        if (!current()) return state
        stage = 'engine'
        const factory = await abortable(importEngine(), controller.signal, options.timeoutMs ?? limits.deadlineMs)
        if (!current()) return state
        options.host.appendChild(mount)
        owned.creating = true
        try { owned.engine = factory(mount, map) }
        finally { owned.creating = false; if (!current()) owned.finish() }
        if (!current()) return state
        const resize = () => {
          if (!current() || owned.frame !== null) return
          owned.frame = requestAnimationFrame(() => {
            owned.frame = null; if (!current()) return
            const box = options.host.getBoundingClientRect()
            owned.engine?.resize(Math.max(1, Math.min(limits.viewportSide, Math.floor(box.width))), Math.max(1, Math.min(limits.viewportSide, Math.floor(box.height))))
          })
        }
        if (typeof ResizeObserver !== 'undefined') { owned.observer = new ResizeObserver(resize); owned.observer.observe(options.host) }
        else { owned.resize = resize; window.addEventListener('resize', resize) }
        await abortable(owned.engine.ready, controller.signal, options.timeoutMs ?? limits.deadlineMs)
        if (!current()) return state
        resize(); owned.engine.setVisible(visible)
        if (!current()) return state // A supported injected lease may synchronously stop/dispose/restart.
        if (options.attachControls) {
          if (!owned.engine.camera) throw new MapError('invalid')
          owned.creating = true
          try { owned.controls = options.attachControls({ host: options.host, map, camera: owned.engine.camera, signal: controller.signal }) }
          finally { owned.creating = false; if (!current()) owned.finish() }
          if (!current()) return state
          owned.controls.setEnabled(visible && !inputBlocked)
          if (!current()) return state
        }
        const g = map.map.geometry
        publish({ phase: 'ready', widthPx: map.widthPx, heightPx: map.heightPx,
          spawnFoot: Object.freeze({ x: (map.spawn.cell_x + .5) * g.tile_width_px, y: (map.spawn.cell_y + .5) * g.tile_height_px }) })
      } catch (error) {
        if (!current()) return state
        const next: RuntimeState = error instanceof MapError && error.reason === 'unavailable' ? { phase: 'unavailable' }
          : { phase: 'error', reason: error instanceof MapError && ['invalid', 'timeout', 'load-failed'].includes(error.reason)
            ? error.reason as 'invalid' | 'timeout' | 'load-failed' : stage === 'load' ? 'load-failed' : 'engine-failed' }
        stop()
        if (!disposed && generation === ticket + 1) publish(next)
      }
      return state
    })
    // Reserve before loading publication, including listeners that synchronously stop/dispose/restart.
    flight = task
    publish({ phase: 'loading' })
    return task
  }
  return {
    start, stop,
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    setVisible(value: boolean) {
      visible = value
      const revision = ++visibilityRevision, owned = active
      owned?.controls?.setEnabled(visible && !inputBlocked)
      if (active === owned && revision === visibilityRevision) owned?.engine?.setVisible(value)
    },
    setInputBlocked(value: boolean) { inputBlocked = value; active?.controls?.setEnabled(visible && !inputBlocked) },
    dispose() { if (disposed) return; disposed = true; stop(); publish({ phase: 'disposed' }); listeners.clear() },
    /** Allows a host adapter to await engine teardown before transferring ownership. */
    settled: () => drain,
  }
}
export type WorldRuntime = ReturnType<typeof createWorldRuntime>
