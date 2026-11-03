import type { SessionController } from '../../auth/model/session'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { TransportError } from '../../../shared/api/errors'
import type { WholeCharacterChoice, WholeCharacterRuntime } from '../../../entities/avatar/whole-character'
import { decodeWholeConfig, decodeWholeParticipant, decodeWholeState, sameWholeConfig, type WholeCharacterConfig, type WholeCharacterState } from './model'

export type { WholeCharacterChoice }
/** Structural subset of the producer port. The injected owner is borrowed, never disposed here. */
export type WholeEditorRuntime = Pick<WholeCharacterRuntime, 'choices' | 'isCurrent'>
export type WholeEditorPhase = 'idle' | 'loading' | 'ready' | 'saving' | 'reconciling' | 'unavailable' | 'disposed'
export type WholeEditorNotice = 'load-failed' | 'art-unavailable' | 'invalid-selection' | 'save-failed' | 'saved' | 'saved-refresh-needed' | 'conflict' | 'uncertain' | 'review-server' | null
export interface WholeEditorState {
  readonly phase: WholeEditorPhase
  readonly choices: readonly WholeCharacterChoice[]
  readonly persisted: WholeCharacterState | null
  readonly draft: WholeCharacterConfig | null
  readonly candidate: WholeCharacterState | null
  readonly needsReconciliation: boolean
  readonly notice: WholeEditorNotice
}
export interface WholeCharacterEditorOptions<R extends WholeEditorRuntime = WholeEditorRuntime> {
  readonly auth: Pick<SessionController, 'privacy' | 'request'>
  readonly scope: PrivacyScope
  readonly runtime: R
  readonly onCommitted?: (avatar: WholeCharacterState, scope: PrivacyScope) => void
}
const empty = (phase: WholeEditorPhase): WholeEditorState => Object.freeze({ phase, choices: Object.freeze([]), persisted: null, draft: null, candidate: null, needsReconciliation: false, notice: null })

/** New whole-character state; audited scoped request/reconciliation principles derive from FE13.
 * Never adapts the layered controller and never acquires/disposes the shared asset owner.
 */
export function createWholeCharacterEditorController<R extends WholeEditorRuntime>(options: WholeCharacterEditorOptions<R>) {
  const scope = Object.freeze({ ...options.scope }), listeners = new Set<() => void>()
  let state = empty('idle'), retained: WholeEditorState | null = null, started = false, disposed = false, revoked = false, epoch = 0
  let observed: WholeCharacterState | null = null
  let operation: AbortController | null = null, cleanup = () => {}
  const current = (version = epoch) => started && !disposed && !revoked && version === epoch
    && !!scope.userId && !!scope.eventId && options.auth.privacy.isCurrent(scope) && options.runtime.isCurrent()
  const publish = (patch: Partial<WholeEditorState>) => { state = Object.freeze({ ...state, ...patch }); for (const listener of Array.from(listeners)) listener() }
  const receiptCurrent = (receipt: { readonly generation: number; readonly userId: string | null; readonly eventId: string | null }) =>
    receipt.generation === scope.generation && receipt.userId === scope.userId && receipt.eventId === scope.eventId
  const supported = (config: WholeCharacterConfig) => state.choices.some(choice => sameWholeConfig(choice.config, config))
  function stopWork() { epoch++; operation?.abort(); operation = null }
  function revoke() {
    if (revoked || disposed) return
    revoked = true; retained = null; observed = null; stopWork(); state = empty('unavailable'); publish({})
  }
  cleanup = options.auth.privacy.registerCleanup(revoke)
  function committed(avatar: WholeCharacterState) {
    if (!current()) return
    try { options.onCommitted?.(avatar, scope) } catch { if (current()) publish({ notice: 'saved-refresh-needed' }) }
  }
  async function readServer(reconcile: boolean, warmClean = false) {
    if (!current() || operation) return false
    const version = epoch, flight = new AbortController(); operation = flight
    publish({ phase: reconcile ? 'reconciling' : 'loading', candidate: null, ...(reconcile && !warmClean ? { needsReconciliation: true } : {}) })
    try {
      const result = await options.auth.privacy.run(scope, ({ signal }) => options.auth.request({
        path: `/api/concert/events/${scope.eventId}/me`, method: 'GET', access: 'protected', errorFamily: 'core', signal,
        decode: value => decodeWholeParticipant(value, scope),
      }), flight.signal)
      if (!current(version) || flight.signal.aborted) return false
      const avatar = decodeWholeState(result.data)
      if (!receiptCurrent(result.scope) || !supported(avatar.config) || (observed && (avatar.revision < observed.revision
        || (avatar.revision === observed.revision && !sameWholeConfig(avatar.config, observed.config))))) throw new Error('Invalid current avatar')
      observed = avatar
      if (reconcile && warmClean && state.persisted && avatar.revision === state.persisted.revision && sameWholeConfig(avatar.config, state.persisted.config)) {
        publish({ phase: 'ready', persisted: avatar, candidate: null, needsReconciliation: false, notice: null })
      } else if (reconcile) publish({ phase: 'ready', candidate: avatar, needsReconciliation: true, notice: 'review-server' })
      else publish({ phase: 'ready', persisted: avatar, draft: avatar.config, candidate: null, needsReconciliation: false, notice: null })
      return true
    } catch { if (current(version) && !flight.signal.aborted) publish({ phase: 'ready', notice: 'load-failed', needsReconciliation: reconcile || state.needsReconciliation }); return false }
    finally { if (operation === flight) operation = null }
  }
  async function initialize() {
    const version = epoch, flight = new AbortController(); operation = flight
    publish({ phase: 'loading', notice: null })
    try {
      const choices = await options.runtime.choices(flight.signal)
      if (!current(version) || flight.signal.aborted) return
      const seen = new Set<string>()
      const validated = choices.map(choice => {
        const config = decodeWholeConfig(choice.config)
        if (choice.id !== config.preset_id || seen.has(choice.id) || typeof choice.label !== 'string' || !choice.label.trim() || choice.label.length > 160) throw new Error('Invalid character choice')
        seen.add(choice.id); return Object.freeze({ id: config.preset_id, label: choice.label, config })
      })
      if (validated.length !== 6) throw new Error('Incomplete current collection')
      const saved = retained; retained = null
      publish({ choices: Object.freeze(validated), ...(saved ? { persisted: saved.persisted, draft: saved.draft, needsReconciliation: saved.needsReconciliation, notice: saved.notice } : {}) })
      operation = null; await readServer(saved !== null, !!saved && !saved.needsReconciliation)
    } catch { if (current(version) && !flight.signal.aborted) publish({ phase: 'unavailable', notice: 'art-unavailable' }) }
    finally { if (operation === flight) operation = null }
  }
  function select(id: string) {
    if (!current() || state.phase !== 'ready' || !state.draft) return false
    const choice = state.choices.find(choice => choice.id === id)
    if (!choice) { publish({ notice: 'invalid-selection' }); return false }
    publish({ draft: choice.config, notice: state.needsReconciliation ? state.notice : null }); return true
  }
  return {
    runtime: options.runtime,
    scope,
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    start() {
      if (started || disposed || revoked) return
      started = true
      if (!current()) { revoke(); return }
      void initialize()
    },
    select,
    random() {
      if (!current() || state.phase !== 'ready' || !state.draft) return false
      const choices = state.choices.filter(choice => !sameWholeConfig(choice.config, state.draft!))
      return choices.length ? select(choices[Math.floor(Math.random() * choices.length)]!.id) : false
    },
    reset() {
      if (!current() || state.phase !== 'ready' || !state.persisted || state.candidate) return false
      publish({ draft: state.persisted.config, notice: state.needsReconciliation ? state.notice : null }); return true
    },
    reload: () => readServer(state.needsReconciliation || state.draft !== null),
    retry() { if (!current() || operation || state.phase !== 'unavailable' || state.notice !== 'art-unavailable') return; void initialize() },
    resolveConflict(choice: 'keep-draft' | 'use-server') {
      if (!current() || operation || !state.candidate || (choice !== 'keep-draft' && choice !== 'use-server')) return false
      const avatar = state.candidate
      publish({ persisted: avatar, draft: choice === 'use-server' ? avatar.config : state.draft, candidate: null, needsReconciliation: false, notice: null })
      committed(avatar); return true
    },
    async save() {
      if (!current() || operation || state.phase !== 'ready' || !state.persisted || !state.draft || state.needsReconciliation
        || !supported(state.draft) || sameWholeConfig(state.draft, state.persisted.config)) return false
      const version = epoch, submitted = decodeWholeConfig(state.draft), revision = state.persisted.revision
      const flight = new AbortController(); operation = flight; publish({ phase: 'saving', notice: null })
      try {
        const result = await options.auth.privacy.run(scope, ({ signal }) => options.auth.request({
          path: `/api/concert/events/${scope.eventId}/me/avatar`, method: 'PUT', access: 'protected', errorFamily: 'core', signal,
          body: { json: { expected_revision: revision, config: { ...submitted } } },
          decode: value => { const avatar = decodeWholeState(value); if (avatar.revision !== revision + 1 || !sameWholeConfig(avatar.config, submitted)) throw new Error('Invalid save response'); return avatar },
        }), flight.signal)
        if (!current(version) || flight.signal.aborted) return false
        const avatar = decodeWholeState(result.data)
        if (!receiptCurrent(result.scope) || avatar.revision !== revision + 1 || !sameWholeConfig(avatar.config, submitted)) throw new Error('Invalid save receipt')
        observed = avatar
        publish({ phase: 'ready', persisted: avatar, draft: avatar.config, candidate: null, needsReconciliation: false, notice: 'saved' })
        committed(avatar); return true
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
    dispose() { if (disposed) return; disposed = true; retained = null; observed = null; stopWork(); cleanup(); cleanup = () => {}; state = empty('disposed'); publish({}); listeners.clear() },
  }
}
export type WholeCharacterEditorController<R extends WholeEditorRuntime = WholeEditorRuntime> = ReturnType<typeof createWholeCharacterEditorController<R>>
