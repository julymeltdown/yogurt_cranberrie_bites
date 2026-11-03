import type { WorldAccessBinding, WorldAccessBindingPort } from '../ports'
import type { WorldEntryRequest } from '../world-entry/authority'
import { sameScope } from '../world-entry/authority'
import type { SessionController } from '../../features/auth/model/session'
import type { ConnectionKind, ConnectionStatusView } from '../../features/connection-status/model'
import type { Scheduler } from '../../world/network/WorldTransport'

export interface ValuePort<T> { read(): T; subscribe(listener: () => void): () => void }
/** A validated producer's projection, never a raw response or navigator inference.
 * Its exact captured world release prevents an old event decision from being restamped.
 */
export interface ServiceDecision {
  readonly release: WorldEntryRequest
  readonly kind: 'normal' | 'maintenance' | 'version' | 'unavailable'
}
export interface RecoveryAction {
  readonly release: WorldEntryRequest
  /** Delegate to the EXISTING FE17 owner, never create another connection. */
  reconnect(): Promise<boolean>
}
export interface ConnectionObserverOptions {
  auth: Pick<SessionController, 'session' | 'privacy'>
  world: WorldAccessBindingPort
  /** Browser network hint only. Online does not certify server reachability. */
  connectivity?: ValuePort<'online' | 'offline' | 'unknown'>
  service?: ValuePort<ServiceDecision | null>
  recovery?: ValuePort<RecoveryAction | null>
  pendingMs?: number
  scheduler?: Scheduler
}
const clock: Scheduler = { now: () => performance.now(), set: (ms, fn) => setTimeout(fn, ms), clear: h => clearTimeout(h as ReturnType<typeof setTimeout>) }
const empty = (): ConnectionStatusView => Object.freeze({ kind: 'unavailable', stale: false, pending: false, canRetry: false })

/** Presentation observer only: one current replication subscription and one bounded
 * display deadline. No visibility listener, retry loop, HTTP, socket, mutation or Auth.
 */
export function createConnectionObserver(options: ConnectionObserverOptions) {
  const scheduler = options.scheduler ?? clock
  const delay = Number.isFinite(options.pendingMs) ? Math.max(100, Math.min(30000, options.pendingMs!)) : 8000
  const listeners = new Set<() => void>()
  let state = empty(), started = false, disposed = false, binding: WorldAccessBinding | null = null
  let stopView = () => {}, stopAbort = () => {}, stops: (() => void)[] = []
  let timer: unknown = null, expired = false, retrying = false, failed = false, attempt = 0
  let authority: string | null = null, stale = false
  const live = (value: WorldAccessBinding) => !disposed && !value.release.signal.aborted
    && options.auth.session.isCurrent(value.release.scope) && options.auth.privacy.isCurrent(value.release.privacyScope)
    && sameScope(value.release.scope, value.release.privacyScope)
    && value.release.bootstrap.participant.userId === value.release.scope.userId
    && value.release.bootstrap.participant.eventId === value.release.scope.eventId
    && value.release.bootstrap.event.eventId === value.release.scope.eventId
    && !['DRAFT', 'ARCHIVED'].includes(value.release.bootstrap.event.phase)
    && value.release.bootstrap.worldAvailable && value.release.bootstrap.capabilities.world.available
  const emit = (next: ConnectionStatusView) => {
    if (state.kind === next.kind && state.stale === next.stale && state.pending === next.pending && state.canRetry === next.canRetry) return
    state = Object.freeze(next)
    for (const fn of Array.from(listeners)) { try { fn() } catch { /* Independent views cannot block privacy withdrawal. */ } }
  }
  function clearTimer() { if (timer !== null) scheduler.clear(timer); timer = null }
  function detach() {
    ++attempt; stopView(); stopView = () => {}; stopAbort(); stopAbort = () => {}; binding = null
    clearTimer(); expired = retrying = failed = stale = false; authority = null
  }
  function action() {
    const candidate = options.recovery?.read()
    return binding && live(binding) && candidate?.release === binding.release && typeof candidate.reconnect === 'function' ? candidate : null
  }
  function update() {
    const current = binding
    if (!current || !live(current)) { detach(); emit(empty()); return }
    const view = current.replication.read()
    const valid = view && sameScope(view.scope, current.release.scope)
      && (!view.baseline || view.baseline.eventId === view.scope.eventId && view.baseline.selfUserId === view.scope.userId)
    if (!live(current) || current !== binding) return
    const decision = options.service?.read()
    const service = decision?.release === current.release && ['normal', 'maintenance', 'version', 'unavailable'].includes(decision.kind) ? decision.kind : 'normal'
    let kind: ConnectionKind
    if (service !== 'normal') kind = service
    else if (valid && view.problem === 'map-version') kind = 'version'
    else if (options.connectivity?.read() === 'offline') kind = 'offline'
    else if (valid && (!view.visible || view.phase === 'suspended')) kind = 'suspended'
    else if (valid && view.phase === 'resync') kind = 'resync'
    else if (valid && ['live', 'switching'].includes(view.phase) && view.baseline && (view.problem === null || view.problem === 'room-unavailable')) kind = 'ready'
    else if (!failed && (!valid || ['idle', 'ticket', 'opening', 'authenticating', 'switching'].includes(view.phase) || retrying)) kind = 'reconnecting'
    else kind = 'unavailable'
    const baseline = valid ? view.baseline : null
    const stamp = baseline ? JSON.stringify([baseline.streamId, baseline.roomEpoch, baseline.roomId, baseline.requestId, baseline.cause]) : null
    if (kind !== 'ready' && authority !== null) stale = true
    if (kind === 'ready') {
      // FE16's live-state notification precedes the replacement snapshot. Neither
      // that notification nor an old baseline with a newer delta clears stale.
      if (stale && (stamp === authority || !valid || view.phase !== 'live')) kind = 'resync'
      else { authority = stamp; stale = false; expired = failed = false }
    }
    const waiting = kind === 'reconnecting' || kind === 'resync'
    if (waiting && !expired && timer === null) {
      const captured = current
      timer = scheduler.set(delay, () => { timer = null; if (binding !== captured || !live(captured)) return; expired = true; retrying = false; update() })
    } else if (!waiting) clearTimer()
    if (waiting && expired) kind = 'unavailable'
    const canRetry = kind === 'unavailable' && !retrying && !!action() && valid
      && view.visible && ['closed', 'idle'].includes(view.phase) && !['map-version', 'authority', 'sink'].includes(view.problem ?? '')
    emit({ kind, stale, pending: waiting && !expired, canRetry: !!canRetry })
  }
  function sync() {
    if (disposed || !started) return
    const next = options.world.read()
    if (!next || !live(next)) { detach(); emit(empty()); return }
    if (!binding || next.release !== binding.release || next.replication !== binding.replication) {
      detach(); binding = next
      const captured = next
      const changed = () => { if (binding === captured) update() }
      captured.release.signal.addEventListener('abort', changed, { once: true })
      stopAbort = () => captured.release.signal.removeEventListener('abort', changed)
      const remove = captured.replication.subscribe(changed)
      if (binding === captured && live(captured)) stopView = remove
      else remove()
    }
    update()
  }
  return {
    read: () => state,
    subscribe(fn: () => void) { if (disposed) return () => {}; listeners.add(fn); return () => { listeners.delete(fn) } },
    start() {
      if (started || disposed) return
      started = true
      stops = [options.auth.privacy.registerCleanup(() => { detach(); emit(empty()) }), options.auth.privacy.subscribe(sync), options.world.subscribe(sync)]
      for (const port of [options.connectivity, options.service, options.recovery]) if (port) stops.push(port.subscribe(sync))
      sync()
    },
    async retry(): Promise<boolean> {
      sync()
      if (!state.canRetry || retrying || !binding) return false
      const recovery = action(), captured = binding
      if (!recovery) return false
      const ticket = ++attempt
      retrying = true; expired = failed = false; clearTimer(); update()
      let accepted = false
      try { accepted = await recovery.reconnect() } catch { /* Fixed unavailable copy only. */ }
      if (disposed || binding !== captured || !live(captured) || ticket !== attempt) return false
      retrying = false; failed = !accepted; update()
      return accepted
    },
    dispose() { if (disposed) return; disposed = true; detach(); for (const stop of stops) stop(); stops = []; emit(empty()); listeners.clear() },
  }
}
export type ConnectionObserver = ReturnType<typeof createConnectionObserver>
