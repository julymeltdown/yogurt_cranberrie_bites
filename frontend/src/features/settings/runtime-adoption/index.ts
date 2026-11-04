import type { RuntimePreferencesPort } from '../SettingsPage'
import { preferencesSchema, type Preferences } from '../../me/model/contract'
import type { ThemeState } from '../../theme/model/controller'
import type { ThemeId } from '../../theme/model/preferences'
import type { PrivacyScope } from '../../../shared/query/privacy'
import type { SessionController } from '../../auth/model/session'

/** Borrowed effects on the incumbent engine; abort must synchronously withdraw them.
 * An ignored-abort acquisition must return its own disposable lease, never mutate a successor. */
export interface RuntimeEffectLease { dispose(): void }
export interface RuntimeEffects {
  readonly motion: boolean
  readonly lowPower: boolean
  acquire(value: Readonly<{ reducedMotion: boolean; lowPower: boolean }>, signal: AbortSignal): RuntimeEffectLease | Promise<RuntimeEffectLease>
}
/** Optional existing audio source only. No source means enable-sound is unavailable.
 * acquire is called synchronously from apply, preserving the explicit browser gesture. */
export interface RuntimeSound {
  acquire(signal: AbortSignal): RuntimeEffectLease | Promise<RuntimeEffectLease>
}
export interface RuntimeTheme {
  read(): ThemeState
  select(theme: ThemeId, signal?: AbortSignal): Promise<void>
}
export interface MotionPreference {
  readonly matches: boolean
  addEventListener(type: 'change', listener: () => void): void
  removeEventListener(type: 'change', listener: () => void): void
}
export interface RuntimeAdoptionOptions {
  readonly privacy: Pick<SessionController['privacy'], 'isCurrent' | 'registerCleanup'>
  readonly scope: PrivacyScope
  /** Exact presentation release lifetime, not a freshly captured/relabelled lease. */
  readonly signal: AbortSignal
  readonly effects?: RuntimeEffects
  readonly sound?: RuntimeSound
  /** Existing ThemeController with external-signal cancellation support. */
  readonly theme?: RuntimeTheme
  readonly motion?: MotionPreference
  readonly timeoutMs?: number
}
export class RuntimePreferenceError extends Error {
  constructor(readonly reason: 'unavailable' | 'failed' | 'cancelled' | 'timeout') {
    super(`Runtime preferences ${reason}.`); this.name = 'RuntimePreferenceError'
  }
}
export interface RuntimeAdoptionState {
  readonly phase: 'idle' | 'applying' | 'applied' | 'failed' | 'disposed'
  readonly reason?: RuntimePreferenceError['reason']
  readonly reducedMotion: boolean
  readonly soundEnabled: boolean
}
const sameScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId
  && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const release = (lease: RuntimeEffectLease | null) => { try { lease?.dispose() } catch { /* Release independent effects too. */ } }

/** Explicit FE34 consumer. Creates no game, connection, identity, storage or server setting.
 * Construction is inert: even a saved sound=true cannot start audio. */
export function createRuntimePreferencesAdapter(options: RuntimeAdoptionOptions) {
  const { privacy } = options, scope = Object.freeze({ ...options.scope })
  const media = options.motion ?? (typeof window !== 'undefined' && window.matchMedia
    ? window.matchMedia('(prefers-reduced-motion: reduce)') : undefined)
  let disposed = false, revision = 0, unregister = () => {}
  let state: RuntimeAdoptionState = Object.freeze({ phase: 'idle', reducedMotion: media?.matches ?? false, soundEnabled: false })
  type Operation = { controller: AbortController; preferences: Readonly<Preferences>; effects: RuntimeEffectLease | null;
    sound: RuntimeEffectLease | null; effectRevision: number; effectFlight: Promise<void>; ticket: number; detach(): void }
  let active: Operation | null = null
  const current = (operation: Operation) => !disposed && active === operation && !operation.controller.signal.aborted
    && !options.signal.aborted && privacy.isCurrent(scope)
  const publish = (next: RuntimeAdoptionState) => { state = Object.freeze(next) }
  function stop(operation: Operation) {
    if (active === operation) active = null
    operation.controller.abort(); operation.detach()
    const effects = operation.effects, sound = operation.sound
    operation.effects = null; operation.sound = null
    release(sound); release(effects)
  }
  function dispose() {
    if (disposed) return
    disposed = true; ++revision
    if (active) stop(active)
    unregister(); options.signal.removeEventListener('abort', dispose)
    media?.removeEventListener('change', osChanged)
    publish({ phase: 'disposed', reducedMotion: media?.matches ?? false, soundEnabled: false })
  }
  function assert(operation: Operation) {
    if (!current(operation)) throw new RuntimePreferenceError('cancelled')
  }
  async function bounded<T>(pending: Promise<T>, operation: Operation): Promise<T> {
    const signal = operation.controller.signal
    let timer: ReturnType<typeof setTimeout> | undefined, abort = () => {}
    const stopped = new Promise<never>((_, reject) => {
      abort = () => reject(new RuntimePreferenceError('cancelled'))
      signal.addEventListener('abort', abort, { once: true })
      if (signal.aborted) abort()
      timer = setTimeout(() => reject(new RuntimePreferenceError('timeout')), options.timeoutMs ?? 15000)
    })
    try { const result = await Promise.race([pending, stopped]); assert(operation); return result }
    finally { clearTimeout(timer); signal.removeEventListener('abort', abort) }
  }
  function acquire(work: () => RuntimeEffectLease | Promise<RuntimeEffectLease>, operation: Operation,
    accept: (lease: RuntimeEffectLease) => void, valid = () => current(operation)) {
    // Invoke now (sound needs the browser gesture), then reserve before any await boundary.
    let pending: RuntimeEffectLease | Promise<RuntimeEffectLease>
    try { assert(operation); pending = work() } catch (error) { return Promise.reject(error) }
    return Promise.resolve(pending).then(lease => {
      if (!lease || typeof lease.dispose !== 'function') throw new RuntimePreferenceError('failed')
      if (!current(operation)) { release(lease); throw new RuntimePreferenceError('cancelled') }
      if (!valid()) { release(lease); return }
      accept(lease)
    })
  }
  function applyEffects(operation: Operation) {
    const ticket = ++operation.effectRevision, reducedMotion = operation.preferences.reduced_motion || Boolean(media?.matches)
    const effects = options.effects
    if (!effects || (!effects.motion && reducedMotion) || (!effects.lowPower && operation.preferences.low_power)) {
      operation.effectFlight = Promise.reject(new RuntimePreferenceError('unavailable')); return operation.effectFlight
    }
    const old = operation.effects; operation.effects = null; release(old)
    operation.effectFlight = acquire(() => effects.acquire({ reducedMotion, lowPower: operation.preferences.low_power }, operation.controller.signal), operation,
      lease => { operation.effects = lease }, () => current(operation) && ticket === operation.effectRevision)
      .then(() => { assert(operation); if (ticket === operation.effectRevision) publish({ ...state, reducedMotion }) })
      .catch(error => { if (current(operation) && ticket !== operation.effectRevision) return; throw error })
    return operation.effectFlight
  }
  async function settleEffects(operation: Operation) {
    let ticket: number
    do { ticket = operation.effectRevision; await operation.effectFlight; assert(operation) } while (ticket !== operation.effectRevision)
  }
  function fail(operation: Operation, error: unknown) {
    if (!current(operation)) return
    stop(operation)
    if (!active && operation.ticket === revision && !disposed) publish({ phase: 'failed', reason: error instanceof RuntimePreferenceError ? error.reason : 'failed',
      reducedMotion: Boolean(media?.matches), soundEnabled: false })
  }
  function osChanged() {
    const operation = active
    if (!operation || !current(operation)) return
    // OS changes only update motion effects. They never start or restart audio or a theme load.
    const pending = applyEffects(operation), ticket = operation.effectRevision
    void bounded(pending, operation).catch(error => { if (ticket === operation.effectRevision) fail(operation, error) })
  }
  const port: RuntimePreferencesPort = {
    async apply(value, expected, signal) {
      // A stale caller must not revoke a newer valid application on this shell.
      if (disposed || options.signal.aborted || signal.aborted || !sameScope(scope, expected) || !privacy.isCurrent(scope)) throw new RuntimePreferenceError('cancelled')
      const ticket = ++revision
      if (active) stop(active)
      if (ticket !== revision) throw new RuntimePreferenceError('cancelled')
      if (disposed || options.signal.aborted || signal.aborted || !privacy.isCurrent(scope)) throw new RuntimePreferenceError('cancelled')
      const parsed = preferencesSchema.safeParse(value)
      const effects = options.effects, theme = options.theme
      if (!parsed.success || !effects || !theme || (!effects.motion && (parsed.data.reduced_motion || Boolean(media?.matches)))
        || (!effects.lowPower && parsed.data.low_power) || (parsed.data.sound_enabled && !options.sound)) {
        publish({ phase: 'failed', reason: 'unavailable', reducedMotion: Boolean(media?.matches), soundEnabled: false })
        throw new RuntimePreferenceError('unavailable')
      }
      // A dispose/abort callback may synchronously reenter apply while releasing the former lease.
      if (ticket !== revision) throw new RuntimePreferenceError('cancelled')
      const operation: Operation = { controller: new AbortController(), preferences: Object.freeze({ ...parsed.data }), effects: null, sound: null,
        effectRevision: 0, effectFlight: Promise.resolve(), ticket, detach: () => signal.removeEventListener('abort', abort) }
      const abort = () => {
        if (active !== operation) return
        stop(operation)
        if (!active && ticket === revision && !disposed) publish({ phase: 'idle', reducedMotion: Boolean(media?.matches), soundEnabled: false })
      }
      active = operation
      signal.addEventListener('abort', abort, { once: true })
      publish({ phase: 'applying', reducedMotion: Boolean(media?.matches), soundEnabled: false })
      try {
        const sound = operation.preferences.sound_enabled
          ? acquire(() => options.sound!.acquire(operation.controller.signal), operation, lease => { operation.sound = lease }) : Promise.resolve()
        // Attach rejection handlers to every operation before invoking another injected callback.
        const effects = applyEffects(operation)
        // Observe the initial acquisition too, even if an OS event synchronously replaced it.
        const pending = Promise.all([sound, effects.then(() => settleEffects(operation))])
        await bounded(pending, operation)
        assert(operation)
        await bounded(theme.select(operation.preferences.theme, operation.controller.signal), operation)
        assert(operation)
        const actual = theme.read()
        if (actual.selected !== operation.preferences.theme || actual.phase !== 'ready') throw new RuntimePreferenceError('failed')
        publish({ phase: 'applied', reducedMotion: operation.preferences.reduced_motion || Boolean(media?.matches), soundEnabled: !!operation.sound })
      } catch (error) {
        fail(operation, error)
        throw error instanceof RuntimePreferenceError ? error : new RuntimePreferenceError('failed')
      }
    },
  }
  if (options.signal.aborted || !privacy.isCurrent(scope)) dispose()
  else {
    unregister = privacy.registerCleanup(dispose)
    options.signal.addEventListener('abort', dispose, { once: true })
    media?.addEventListener('change', osChanged)
  }
  return Object.freeze({ apply: port.apply, read: () => state, dispose })
}
export type RuntimePreferencesAdapter = ReturnType<typeof createRuntimePreferencesAdapter>
