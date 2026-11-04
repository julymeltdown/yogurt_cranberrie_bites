import { createMuralHttp, createMuralSync, createMuralToolsHttp, type MuralSyncController, type MuralAuthority, type MuralAuthorityPort } from '../../features/mural/sync'
import { bindingSchema, sameBinding, sameScope } from '../../features/mural/sync/contract'
import type { HttpClient } from '../../shared/api/http'
import { createMuralTextureSink, type MuralSink } from '../../world/mural'
import type { MuralEntryOptions, MuralEntryRelease } from './ports'
import { invalidateMuralEligibility } from '../world-entry/mural-production/refresh'

export interface MuralEntryView {
  readonly phase: 'available' | 'unavailable' | 'disposed'
  readonly sync: MuralSyncController | null
  readonly generation: number
}
export const unavailableMuralEntry: MuralEntryView = Object.freeze({ phase: 'unavailable', sync: null, generation: 0 })

/** One owner per canonical shell privacy scope. Release replacement NEVER replaces its editor/command. */
export function createMuralEntry({ auth, scope: supplied, clock, receiptNow }: MuralEntryOptions) {
  const scope = Object.freeze({ ...supplied }), privacy = auth.privacy, session = auth.session
  const request = auth.request.bind(auth)
  let ended = false, sync: MuralSyncController | null = null, release: MuralEntryRelease | null = null, sourceRelease: MuralEntryRelease | null = null
  let authorityValue: MuralAuthority | null = null, delegate: MuralSink | null = null
  let requests = new AbortController(), stops: (() => void)[] = [], stopPrivacy = () => {}
  let view = unavailableMuralEntry, viewGeneration = 0
  const views = new Set<() => void>(), authorityListeners = new Set<() => void>(), failures = new Set<() => void>(), withdrawals = new Set<() => void>()
  const safe = (fn: () => void) => { try { fn() } catch { /* Independent cleanup and observers must all run. */ } }
  const emit = (listeners: Set<() => void>) => { for (const fn of Array.from(listeners)) safe(fn) }
  const current = () => !ended && scope.gate === 'open' && !!scope.userId && !!scope.eventId
    && privacy.isCurrent(scope) && session.isCurrent(scope) && sameScope(scope, privacy.capture())
  function update() {
    const next = current() && authorityValue ? sync : null, phase = ended ? 'disposed' : next ? 'available' : 'unavailable'
    if (view.phase !== phase || view.sync !== next || view.generation !== viewGeneration) { view = Object.freeze({ phase, sync: next, generation: viewGeneration }); emit(views) }
  }
  function clearSink() { const old = delegate; delegate = null; safe(() => old?.dispose()) }
  function abortRequests() { const old = requests; requests = new AbortController(); old.abort() }
  function clearRelease() {
    release = sourceRelease = null; authorityValue = null; ++viewGeneration; emit(withdrawals); abortRequests()
    for (const stop of stops.splice(0).reverse()) safe(stop)
    clearSink(); emit(authorityListeners); update()
  }
  const authority: MuralAuthorityPort = {
    read() {
      if (!current() || !release || release.signal.aborted) return null
      try {
        const value = release.authority.read()
        return value && sameBinding(value.binding, release.binding) && typeof value.writable === 'boolean'
          ? Object.freeze({ binding: release.binding, writable: value.writable }) : null
      } catch { return null }
    },
    subscribe(fn) { authorityListeners.add(fn); return () => { authorityListeners.delete(fn) } },
  }
  const sink: MuralSink = {
    replace(frame) {
      if (!frame) { delegate?.replace(null); return }
      if (!current() || !authorityValue || !sameBinding(frame.binding, authorityValue.binding) || !delegate) throw new Error('Mural scene unavailable')
      delegate.replace(frame)
    },
    dispose: clearSink,
    subscribeFailure(fn) { failures.add(fn); return () => { failures.delete(fn) } },
  }
  const http: HttpClient = {
    request(input) {
      const captured = release, flight = requests
      if (!current() || !captured || !authorityValue || captured.signal.aborted) return Promise.reject(new Error('Mural release unavailable'))
      const signal = AbortSignal.any([flight.signal, captured.signal, ...(input.signal ? [input.signal] : [])])
      return privacy.run(scope, async context => {
        if (release !== captured || signal.aborted) throw new Error('Mural release changed')
        try {
          const result = await request({ ...input, signal: context.signal })
          if (release !== captured || signal.aborted || !current()) throw new Error('Mural release changed')
          return result
        } finally {
          // Reset snapshots and command outcomes refresh admission independently.
          // Tools and delta reads must not recursively invalidate their own polling.
          if (release === captured && current() && (input.method !== 'GET' || (/\/mural\/(?:snapshot|commands|actions)(?:\/|$)/.test(input.path) || input.path.endsWith('/commands')))) {
            // Let canonical HTTP and its command/read consumer finish the response
            // microtasks before withdrawing writes; owner authority changes abort
            // in-flight requests, including this completed response otherwise.
            globalThis.setTimeout(() => { if (release === captured && current()) invalidateMuralEligibility(auth, scope) }, 0)
          }
        }
      }, signal)
    },
    // Feature HTTP never calls refresh. Session refresh ownership stays with canonical Auth.
    refreshSession: async () => { throw new Error('Use canonical Auth refresh ownership') },
  }
  function dispose() {
    if (ended) return
    ended = true; clearRelease(); safe(stopPrivacy); safe(() => sync?.dispose())
    update(); views.clear(); authorityListeners.clear(); failures.clear(); withdrawals.clear()
  }
  if (current()) {
    try {
      stopPrivacy = privacy.registerCleanup(dispose)
      const adapter = createMuralHttp({ http, privacy, authority, ...(receiptNow ? { now: receiptNow } : {}) })
      sync = createMuralSync({ privacy, authority, sink, ...adapter, tools: createMuralToolsHttp({ http, privacy, authority, ...(receiptNow ? { now: receiptNow } : {}) }), ...(clock ? { clock } : {}) })
      sync.start()
    } catch { dispose() }
  } else dispose()

  function bind(input: MuralEntryRelease | null) {
    if (input === sourceRelease && current() && release && !release.signal.aborted) return
    clearRelease()
    if (!input || !current() || input.signal.aborted || !sameScope(scope, input.scope)) return
    const parsed = bindingSchema.safeParse(input.binding)
    if (!parsed.success || parsed.data.eventId !== scope.eventId || typeof input.scene?.onClose !== 'function') return
    // Capture source objects AND methods. Caller mutation cannot retarget an admitted lease.
    const binding = Object.freeze(parsed.data), signal = input.signal
    const read = input.authority.read.bind(input.authority), subscribe = input.authority.subscribe.bind(input.authority)
    const source = input.scene, scene = Object.freeze({ texture: source.texture.bind(source), object: source.object.bind(source),
      onFrame: source.onFrame.bind(source), onClose: source.onClose.bind(source) })
    const captured = Object.freeze({ scope, binding, signal, authority: Object.freeze({ read, subscribe }), scene })
    release = captured; sourceRelease = input
    const live = () => current() && release === captured && !signal.aborted
    const withdraw = () => { if (release === captured) clearRelease() }
    const register = (make: () => () => void) => {
      const stop = make()
      if (live()) stops.push(stop); else safe(stop)
    }
    function changed() {
      if (!live()) return
      let next: MuralAuthority | null = null
      try { const value = read(); if (value && sameBinding(value.binding, binding) && typeof value.writable === 'boolean') next = Object.freeze({ binding, writable: value.writable }) }
      catch { withdraw(); return }
      if (!sameBinding(authorityValue?.binding ?? null, next?.binding ?? null) || authorityValue?.writable !== next?.writable) abortRequests()
      if (authorityValue && !next) ++viewGeneration
      authorityValue = next
      if (!next) { emit(withdrawals); safe(() => delegate?.replace(null)) }
      emit(authorityListeners); update()
    }
    try {
      delegate = createMuralTextureSink(scene)
      register(() => delegate!.subscribeFailure!(() => { if (live()) { withdraw(); emit(failures) } }))
      register(() => scene.onClose(withdraw))
      register(() => subscribe(changed))
      if (!live()) { withdraw(); return }
      signal.addEventListener('abort', withdraw, { once: true }); stops.push(() => signal.removeEventListener('abort', withdraw))
      changed()
    } catch { withdraw() }
  }
  return Object.freeze({ auth, scope, read: () => view, bind,
    subscribe(fn: () => void) { if (ended) return () => {}; views.add(fn); return () => { views.delete(fn) } },
    /** Evict retained DOM canvas pixels before controller/store notifications or external cleanup. */
    registerWithdrawal(fn: () => void) { if (ended) { safe(fn); return () => {} }; withdrawals.add(fn); return () => { withdrawals.delete(fn) } },
    dispose, isDisposed: () => ended })
}
export type MuralEntry = ReturnType<typeof createMuralEntry>
