import type { MuralEntryRelease } from '../../app/mural-entry/ports'
import type { PrivacyPort } from '../../features/mural/editor/contract'
import { bindingSchema, sameBinding, sameScope, type MuralAuthorityPort } from '../../features/mural/sync/contract'
import type { WorldReplicationRelease } from '../composition'
import type { IncumbentMuralScene } from '../runtime'
import type { MuralSceneObject, MuralSceneTexture } from '../mural/sink'

/** An explicit in-process qualification from the incumbent owner, never inferred from map labels.
 * The supplier owns admission, reviewed anchor/key and canonical server write eligibility.
 */
export interface MuralEntryQualification {
  readonly scope: MuralEntryRelease['scope']
  readonly binding: MuralEntryRelease['binding']
  readonly signal: AbortSignal
  readonly authority: MuralAuthorityPort
  readonly scene: IncumbentMuralScene
  readonly anchor: Parameters<IncumbentMuralScene>[0]
  readonly textureKey: string
}

/** Borrow one world and scene; own only the published mural release and its subordinate leases.
 * Withdrawal is terminal for a qualification. Reconnection needs an explicit fresh bind.
 */
export function createMuralEntryLifecycle({ privacy, world }: {
  readonly privacy: PrivacyPort
  readonly world: Pick<WorldReplicationRelease, 'read' | 'subscribe'>
}) {
  const scope = Object.freeze({ ...privacy.capture() })
  const readWorld = world.read.bind(world)
  const listeners = new Set<() => void>(), consumed = new WeakSet<MuralEntryQualification>()
  let disposed = false, withdrawing = false, revision = 0, stopWorld = () => {}, stopPrivacy = () => {}
  type Candidate = {
    input: MuralEntryQualification; release: MuralEntryRelease | null; controller: AbortController
    valid(): boolean; stops: Set<() => void>; resources: Set<() => void>
    closes: Set<() => void>; authorities: Set<() => void>
  }
  let active: Candidate | null = null
  const safe = (work: () => void) => { try { work() } catch { /* Independent teardown must continue. */ } }
  const emit = (set: Set<() => void>) => { for (const fn of Array.from(set)) safe(fn) }
  function clear(candidate: Candidate | null) {
    if (!candidate || active !== candidate) return
    active = null; withdrawing = true // Fence stale reads and reentrant callbacks before abort/notification/cleanup.
    candidate.controller.abort()
    emit(candidate.closes); emit(candidate.authorities); emit(listeners)
    for (const stop of Array.from(candidate.stops).reverse()) safe(stop)
    for (const dispose of Array.from(candidate.resources).reverse()) safe(dispose)
    candidate.stops.clear(); candidate.resources.clear(); candidate.closes.clear(); candidate.authorities.clear()
    withdrawing = false
  }
  function revoke(candidate: Candidate | null) {
    if (!candidate || active !== candidate) return
    ++revision; clear(candidate)
  }
  const live = (candidate: Candidate) => active === candidate && !disposed && !candidate.controller.signal.aborted && candidate.valid()
  function read(): MuralEntryRelease | null {
    const candidate = active
    if (candidate && !live(candidate)) revoke(candidate)
    return active?.release ?? null
  }
  function dispose() {
    if (disposed) return
    disposed = true; ++revision; clear(active)
    safe(stopWorld); safe(stopPrivacy); listeners.clear()
  }
  function changed() {
    const candidate = active
    if (!candidate) return
    if (!live(candidate)) { revoke(candidate); return }
    emit(candidate.authorities)
  }
  try {
    stopWorld = world.subscribe(changed)
    if (disposed) safe(stopWorld)
    stopPrivacy = privacy.registerCleanup(dispose)
    if (disposed) safe(stopPrivacy)
  } catch { dispose() }

  function bind(input: MuralEntryQualification | null): boolean {
    if (disposed || withdrawing) return false
    if (input && active?.input === input) return read() !== null
    const turn = ++revision
    clear(active)
    if (!input || turn !== revision || disposed || consumed.has(input)) return false
    consumed.add(input)
    let candidate: Candidate | null = null
    try {
      const binding = Object.freeze(bindingSchema.parse(input.binding)), anchor = Object.freeze({ ...input.anchor })
      const signal = input.signal, key = input.textureKey, makeScene = input.scene
      const authorityRead = input.authority.read.bind(input.authority), subscribe = input.authority.subscribe.bind(input.authority)
      if (!sameScope(scope, input.scope) || scope.gate !== 'open' || !scope.userId || !scope.eventId || binding.eventId !== scope.eventId
        || ![anchor.x, anchor.y, anchor.width, anchor.height, anchor.depth].every(Number.isFinite)
        || anchor.width <= 0 || anchor.height <= 0 || anchor.width > 2048 || anchor.height > 2048 || !/^[a-zA-Z0-9_-]{1,80}$/.test(key)) return false
      const valid = () => {
        try {
          if (signal.aborted || !privacy.isCurrent(scope) || !sameScope(scope, privacy.capture())) return false
          const view = readWorld(), value = authorityRead()
          return !!view && view.phase === 'live' && view.visible && !!view.baseline
            && view.scope.userId === scope.userId && view.scope.eventId === scope.eventId && view.scope.generation === scope.generation
            && sameBinding(binding, view.baseline) && !!value && sameBinding(binding, value.binding) && typeof value.writable === 'boolean'
        } catch { return false }
      }
      if (!valid() || turn !== revision || disposed) return false
      const owned: Candidate = { input, release: null, controller: new AbortController(), valid,
        stops: new Set(), resources: new Set(), closes: new Set(), authorities: new Set() }
      candidate = active = owned
      const assertLive = () => { if (!live(owned)) { revoke(owned); throw new Error('Mural entry release withdrawn') } }
      const register = (make: () => () => void) => {
        const stop = make()
        if (live(owned)) owned.stops.add(stop); else { safe(stop); revoke(owned) }
      }
      const source = makeScene(anchor, key)
      assertLive()
      const texture = source.texture.bind(source), object = source.object.bind(source), onFrame = source.onFrame.bind(source), onClose = source.onClose.bind(source)
      const textures = new Map<MuralSceneTexture, MuralSceneTexture>()
      const borrowed = <T>(work: () => T): T => {
        assertLive()
        try { return work() } catch (error) { revoke(owned); throw error }
      }
      function resource<T extends MuralSceneObject>(value: T): T {
        const remove = value.dispose.bind(value)
        let ended = false
        const dispose = () => { if (ended) return; ended = true; owned.resources.delete(dispose); remove() }
        owned.resources.add(dispose)
        if (!live(owned)) { safe(dispose); assertLive() }
        return { ...value, dispose }
      }
      const scene: MuralEntryRelease['scene'] = Object.freeze({
        texture() {
          assertLive()
          const value = borrowed(texture), replace = value.replace.bind(value), remove = value.dispose.bind(value)
          let wrapped: MuralSceneTexture | null = null, ended = false
          wrapped = resource({ replace(rgba: Uint8ClampedArray) {
            if (ended) throw new Error('Mural texture disposed')
            borrowed(() => replace(rgba))
          }, dispose() { ended = true; if (wrapped) textures.delete(wrapped); remove() } })
          textures.set(wrapped, value); return wrapped
        },
        object(value: MuralSceneTexture) {
          assertLive(); const underlying = textures.get(value)
          if (!underlying) throw new Error('Unowned mural texture')
          return resource(borrowed(() => object(underlying)))
        },
        onFrame(listener: () => void) {
          assertLive()
          const stop = borrowed(() => onFrame(() => { if (live(owned)) safe(listener); else revoke(owned) }))
          let ended = false
          const remove = () => { if (ended) return; ended = true; owned.stops.delete(remove); stop() }
          if (live(owned)) owned.stops.add(remove); else { safe(remove); assertLive() }
          return remove
        },
        onClose(listener: () => void) {
          if (!live(owned)) { safe(listener); return () => {} }
          owned.closes.add(listener); return () => { owned.closes.delete(listener) }
        },
      })
      const authority: MuralAuthorityPort = Object.freeze({
        read() {
          if (!live(owned)) { revoke(owned); return null }
          try {
            const value = authorityRead()
            if (value && sameBinding(value.binding, binding) && typeof value.writable === 'boolean') return Object.freeze({ binding, writable: value.writable })
          } catch { /* Withdraw on a failed borrowed capability. */ }
          revoke(owned); return null
        },
        subscribe(listener: () => void) {
          if (!live(owned)) return () => {}
          owned.authorities.add(listener); return () => { owned.authorities.delete(listener) }
        },
      })
      register(() => onClose(() => revoke(owned)))
      register(() => subscribe(changed))
      register(() => { const stop = () => revoke(owned); signal.addEventListener('abort', stop, { once: true }); return () => signal.removeEventListener('abort', stop) })
      assertLive()
      owned.release = Object.freeze({ scope, binding, signal: owned.controller.signal, authority, scene })
      emit(listeners)
      return active === owned && live(owned)
    } catch { revoke(candidate); return false }
  }
  return Object.freeze({ bind, read, dispose, isDisposed: () => disposed,
    subscribe(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } } })
}
export type MuralEntryLifecycle = ReturnType<typeof createMuralEntryLifecycle>
