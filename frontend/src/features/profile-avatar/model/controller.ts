import type { PrivacyScope } from '../../../shared/query/privacy'
import { TransportError } from '../../../shared/api/errors'
import { canonicalJson } from '../../../world/map/json'
import { requireMotion, type MotionLayer, type MotionSelection } from '../../../entities/emo-avatar/art'
import type { MotionRuntime } from '../../../entities/emo-avatar/runtime'

export interface MotionAvatarState<C> { readonly revision: number; readonly config: C }
export interface MotionPersistence<C> {
  read(signal: AbortSignal): Promise<MotionAvatarState<C>>
  save(expectedRevision: number, config: C, signal: AbortSignal): Promise<MotionAvatarState<C>>
}
export type MotionNotice = 'load-failed' | 'art-unavailable' | 'invalid-selection' | 'saved' | 'save-failed' | 'conflict' | 'uncertain' | 'review-server' | 'saved-refresh-needed' | null
type Catalog<C> = Awaited<ReturnType<MotionRuntime<C>['catalog']>>
export interface MotionEditorState<C> {
  readonly phase: 'idle' | 'loading' | 'ready' | 'saving' | 'reconciling' | 'unavailable' | 'disposed'
  readonly catalog: Catalog<C> | null; readonly persisted: MotionAvatarState<C> | null; readonly draft: C | null
  readonly candidate: MotionAvatarState<C> | null; readonly needsReconciliation: boolean; readonly notice: MotionNotice
}
const same = (a: unknown, b: unknown) => canonicalJson(a as Record<string, unknown>) === canonicalJson(b as Record<string, unknown>)

/**
 * WP-08 T-704: the profile avatar editor controller.
 *
 * Carried over unchanged from the concert editor: the draft/persisted
 * separation, save, the conflict and uncertain reconciliation, and the
 * synchronous withdrawal on privacy revocation.
 *
 * Re-keyed: the scope is the profile. The concert controller compared
 * `scope.eventId` in `current()`; a profile avatar has no event, so the gate is
 * the account and its generation. Nothing here reads or forwards an event id.
 */
export function createProfileAvatarEditorController<C>(options: {
  readonly runtime: MotionRuntime<C>; readonly persistence: MotionPersistence<C>
  readonly privacy: { isCurrent(scope: PrivacyScope): boolean; registerCleanup(cleanup: () => void): () => void }
  readonly onCommitted?: (avatar: MotionAvatarState<C>, scope: PrivacyScope) => void
}) {
  const runtime = options.runtime, scope = runtime.scope, listeners = new Set<() => void>()
  const empty = (phase: MotionEditorState<C>['phase']): MotionEditorState<C> => Object.freeze({ phase, catalog: null, persisted: null, draft: null, candidate: null, needsReconciliation: false, notice: null })
  let state = empty('idle'), retained: MotionEditorState<C> | null = null, observed: MotionAvatarState<C> | null = null
  let started = false, disposed = false, revoked = false, epoch = 0, editEpoch = 0, operation: AbortController | null = null
  const current = (version = epoch) => started && !disposed && !revoked && version === epoch && runtime.isCurrent() && options.privacy.isCurrent(scope as PrivacyScope)
  function publish(patch: Partial<MotionEditorState<C>>) { state = Object.freeze({ ...state, ...patch }); for (const fn of Array.from(listeners)) { try { fn() } catch { /* Independent views. */ } } }
  function stopWork() { epoch++; editEpoch++; operation?.abort(); operation = null }
  function revoke() { if (revoked || disposed) return; revoked = true; retained = null; observed = null; stopWork(); state = empty('unavailable'); publish({}) }
  const unregister = options.privacy.registerCleanup(revoke)
  function decode(value: MotionAvatarState<C>) {
    requireMotion(Number.isSafeInteger(value.revision) && value.revision >= 1)
    return Object.freeze({ revision: value.revision, config: runtime.validateConfig(value.config) })
  }
  function committed(avatar: MotionAvatarState<C>) { if (!current()) return; try { options.onCommitted?.(avatar, scope as PrivacyScope) } catch { if (current()) publish({ notice: 'saved-refresh-needed' }) } }
  async function readServer(reconcile: boolean, warmClean = false) {
    if (!current() || operation) return false
    const version = epoch, flight = new AbortController(); operation = flight
    publish({ phase: reconcile ? 'reconciling' : 'loading', candidate: null, ...(reconcile && !warmClean ? { needsReconciliation: true } : {}) })
    try {
      const avatar = decode(await options.persistence.read(flight.signal))
      if (!current(version) || flight.signal.aborted) return false
      if (observed && (avatar.revision < observed.revision || avatar.revision === observed.revision && !same(avatar.config, observed.config))) throw Error('Stale motion receipt')
      observed = avatar
      if (reconcile && warmClean && state.persisted && avatar.revision === state.persisted.revision && same(avatar.config, state.persisted.config)) {
        publish({ phase: 'ready', persisted: avatar, candidate: null, needsReconciliation: false, notice: null })
      } else if (reconcile) publish({ phase: 'ready', candidate: avatar, needsReconciliation: true, notice: 'review-server' })
      else publish({ phase: 'ready', persisted: avatar, draft: avatar.config, candidate: null, needsReconciliation: false, notice: null })
      return true
    } catch { if (current(version) && !flight.signal.aborted) publish({ phase: 'ready', notice: 'load-failed', needsReconciliation: reconcile || state.needsReconciliation }); return false }
    finally { if (operation === flight) operation = null }
  }
  async function initialize() {
    const version = epoch, flight = new AbortController(); operation = flight; publish({ phase: 'loading', notice: null })
    try {
      const catalog = await runtime.catalog(flight.signal)
      if (!current(version) || flight.signal.aborted) return
      const saved = retained; retained = null
      publish({ catalog, ...(saved ? { persisted: saved.persisted, draft: saved.draft, needsReconciliation: saved.needsReconciliation, notice: saved.notice } : {}) })
      operation = null; await readServer(saved !== null, !!saved && !saved.needsReconciliation)
    } catch { if (current(version) && !flight.signal.aborted) publish({ phase: 'unavailable', notice: 'art-unavailable' }) }
    finally { if (operation === flight) operation = null }
  }
  function selectPreset(id: string) {
    if (!current() || state.phase !== 'ready' || !state.draft) return false
    const preset = state.catalog?.presets.find(p => p.id === id)
    if (!preset) { publish({ notice: 'invalid-selection' }); return false }
    editEpoch++; publish({ draft: runtime.validateConfig(preset.config), notice: state.needsReconciliation ? state.notice : null }); return true
  }
  async function choose(selection: MotionSelection) {
    if (!current() || state.phase !== 'ready' || !state.draft || operation) return false
    const version = epoch, edit = ++editEpoch, flight = new AbortController(), draft = state.draft
    try {
      const next = await runtime.choose(draft, selection, flight.signal)
      if (!current(version) || edit !== editEpoch || operation) return false
      publish({ draft: next, notice: state.needsReconciliation ? state.notice : null }); return true
    } catch { if (current(version) && edit === editEpoch) publish({ notice: 'invalid-selection' }); return false }
  }
  return { kind: 'profile-avatar-editor' as const, runtime, scope, read: () => state,
    subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } },
    start() { if (started || disposed || revoked) return; started = true; if (!current()) { revoke(); return }; void initialize() },
    selectPreset,
    choose,
    random() {
      if (!current() || state.phase !== 'ready' || !state.draft || !state.catalog || operation) return Promise.resolve(false)
      const selected = runtime.authority.selection(state.draft)
      if (selected.mode === 'preset') {
        const presets = state.catalog.presets.filter(preset => !same(preset.config, state.draft))
        return Promise.resolve(presets.length > 0 && selectPreset(presets[Math.floor(Math.random() * presets.length)]!.id))
      }
      // Every coordinate comes from the actual admitted catalog. Keep preset and layered modes explicit.
      const pick = <T,>(values: readonly T[]): T => { requireMotion(values.length > 0); return values[Math.floor(Math.random() * values.length)]! }
      const keys = (kind: MotionLayer) => state.catalog!.items.filter(item => item.kind === kind).map(item => item.key)
      let next: MotionSelection = { mode: 'wardrobe', body: pick(keys('body')), hair: pick(keys('hair')), eyes: pick(keys('eyes')),
        outfit: pick([...keys('outfit'), null]), accessory: pick([...keys('accessory'), null]) }
      // A repeated random sample still offers a different valid choice without unbounded retry.
      if (same(next, selected)) next = { ...next, hair: pick(keys('hair').filter(key => key !== selected.hair)) }
      return choose(next)
    },
    reset() { if (!current() || state.phase !== 'ready' || !state.persisted || state.candidate) return false; editEpoch++; publish({ draft: state.persisted.config, notice: state.needsReconciliation ? state.notice : null }); return true },
    reload: () => readServer(state.needsReconciliation || state.draft !== null),
    retry() { if (current() && !operation && state.phase === 'unavailable') void initialize() },
    resolveConflict(choice: 'keep-draft' | 'use-server') {
      if (!current() || operation || !state.candidate || !['keep-draft', 'use-server'].includes(choice)) return false
      const avatar = state.candidate; editEpoch++
      publish({ persisted: avatar, draft: choice === 'use-server' ? avatar.config : state.draft, candidate: null, needsReconciliation: false, notice: null }); committed(avatar); return true
    },
    async save() {
      if (!current() || operation || state.phase !== 'ready' || !state.persisted || !state.draft || state.needsReconciliation || same(state.draft, state.persisted.config)) return false
      const version = epoch, submitted = runtime.validateConfig(state.draft), revision = state.persisted.revision, flight = new AbortController()
      editEpoch++; operation = flight; publish({ phase: 'saving', notice: null })
      try {
        const avatar = decode(await options.persistence.save(revision, submitted, flight.signal))
        if (!current(version) || flight.signal.aborted) return false
        if (avatar.revision !== revision + 1 || !same(avatar.config, submitted)) throw Error('Invalid motion save receipt')
        observed = avatar; publish({ phase: 'ready', persisted: avatar, draft: avatar.config, candidate: null, needsReconciliation: false, notice: 'saved' }); committed(avatar); return true
      } catch (error) {
        if (current(version) && !flight.signal.aborted) {
          const conflict = error instanceof TransportError && error.status === 409 && error.server?.reason === 'VERSION_CONFLICT'
          const uncertain = !(error instanceof TransportError) || error.uncertain
          publish({ phase: 'ready', needsReconciliation: conflict || uncertain, notice: conflict ? 'conflict' : uncertain ? 'uncertain' : 'save-failed' })
        }
        return false
      } finally { if (operation === flight) operation = null }
    },
    stop() {
      if (!started || disposed) return
      retained = current() && state.persisted && state.draft ? { ...state, needsReconciliation: state.needsReconciliation || state.phase === 'saving', notice: state.phase === 'saving' ? 'uncertain' : state.notice } : null
      started = false; stopWork(); state = empty('idle'); publish({})
    },
    dispose() { if (disposed) return; disposed = true; retained = null; observed = null; stopWork(); unregister(); state = empty('disposed'); publish({}); listeners.clear() },
  }
}
export type ProfileAvatarEditorController<C = unknown> = ReturnType<typeof createProfileAvatarEditorController<C>>
