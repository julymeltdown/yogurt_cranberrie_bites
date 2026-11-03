import type { SessionController } from '../auth/model/session'
import { TransportError } from '../../shared/api/errors'
import type { PrivacyScope } from '../../shared/query/privacy'
import { copyConfig, decodeAvatar, decodeParticipantAvatar, immutable, sameConfig,
  type AvatarCatalog, type AvatarConfig, type AvatarPreviewPort, type AvatarState, type Layer, type PreviewLease } from './contract'

export type EditorPhase = 'idle' | 'loading' | 'ready' | 'saving' | 'reconciling' | 'unavailable' | 'disposed'
export type EditorNotice = 'invalid-selection' | 'load-failed' | 'save-failed' | 'saved' | 'saved-refresh-needed' | 'conflict' | 'uncertain' | 'review-server' | 'preview-unavailable' | null
export interface EditorState {
  readonly phase: EditorPhase
  readonly persisted: AvatarState | null
  readonly draft: AvatarConfig | null
  readonly catalog: AvatarCatalog | null
  readonly candidate: AvatarState | null
  readonly needsReconciliation: boolean
  readonly notice: EditorNotice
  readonly preview: { readonly phase: 'idle' | 'loading' | 'ready' | 'unavailable'; readonly canvas: HTMLCanvasElement | null }
}
export interface AvatarEditorOptions {
  readonly auth: Pick<SessionController, 'privacy' | 'request'>
  /** Create a new lease for each canonical identity/event/privacy generation. */
  readonly createPreview: (scope: PrivacyScope, signal: AbortSignal) => Promise<AvatarPreviewPort>
  /** Integrator updates/invalidates world and profile from this same verified response. No optimistic outfit publication. */
  readonly onCommitted?: (avatar: AvatarState, scope: PrivacyScope) => void
}
const empty = (phase: EditorPhase): EditorState => ({ phase, persisted: null, draft: null, catalog: null, candidate: null,
  needsReconciliation: false, notice: null, preview: { phase: 'idle', canvas: null } })
export function createAvatarEditorController(options: AvatarEditorOptions) {
  let state: EditorState = empty('idle'), scope: PrivacyScope | null = null, started = false, disposed = false, epoch = 0
  let lifetime = new AbortController(), previewFlight = new AbortController(), preview: AvatarPreviewPort | null = null, lease: PreviewLease | null = null
  let operation: AbortController | null = null, removers: (() => void)[] = []
  const listeners = new Set<() => void>()
  const publish = (patch: Partial<EditorState>) => { state = Object.freeze({ ...state, ...patch }); [...listeners].forEach(listener => listener()) }
  const current = (captured: PrivacyScope, version = epoch) => !disposed && version === epoch && options.auth.privacy.isCurrent(captured)
  function clear(next: EditorPhase) {
    epoch++; lifetime.abort(); lifetime = new AbortController(); operation?.abort(); operation = null
    previewFlight.abort(); lease?.dispose(); lease = null; preview?.dispose(); preview = null; scope = null
    state = empty(next)
  }
  async function show(config: AvatarConfig) {
    previewFlight.abort(); previewFlight = new AbortController(); const flight = previewFlight, owner = preview, captured = scope, version = epoch
    lease?.dispose(); lease = null
    if (!owner || !captured || !owner.supports(config)) { publish({ preview: { phase: 'unavailable', canvas: null } }); return }
    publish({ preview: { phase: 'loading', canvas: null } })
    try {
      const next = await owner.render(config, flight.signal)
      if (!current(captured, version) || flight.signal.aborted || preview !== owner) { next.dispose(); return }
      lease = next; publish({ preview: { phase: 'ready', canvas: next.canvas } })
    } catch { if (current(captured, version) && !flight.signal.aborted) publish({ preview: { phase: 'unavailable', canvas: null } }) }
  }
  async function readServer(reconcile: boolean) {
    if (!scope || !current(scope) || operation) return
    const captured = scope, version = epoch, flight = new AbortController(); operation = flight
    publish({ phase: reconcile ? 'reconciling' : 'loading', candidate: null, ...(reconcile ? { needsReconciliation: true } : {}) })
    try {
      const response = await options.auth.privacy.run(captured, ({ signal }) => options.auth.request({
        path: `/api/concert/events/${captured.eventId}/me`, method: 'GET', access: 'protected', errorFamily: 'core', signal,
        decode: value => decodeParticipantAvatar(value, captured),
      }), flight.signal)
      if (!current(captured, version) || flight.signal.aborted) return
      const avatar = response.data
      if (state.persisted && avatar.revision < state.persisted.revision) throw new TransportError('invalid-response')
      if (reconcile) publish({ phase: 'ready', candidate: avatar, notice: 'review-server' })
      else {
        publish({ phase: 'ready', persisted: avatar, draft: avatar.config, notice: null, needsReconciliation: false })
        void show(avatar.config)
      }
    } catch { if (current(captured, version) && !flight.signal.aborted) publish({ phase: 'ready', notice: 'load-failed' }) }
    finally { if (operation === flight) operation = null }
  }
  async function initialize() {
    clear('loading'); const captured = options.auth.privacy.capture(); scope = captured; const version = epoch
    if (!captured.eventId || !current(captured)) { publish({ phase: 'unavailable' }); return }
    publish({ phase: 'loading' })
    try {
      const owner = await options.createPreview(captured, lifetime.signal)
      if (!current(captured, version)) { owner.dispose(); return }
      preview = owner
      const catalog = immutable(structuredClone(owner.catalog))
      publish({ catalog })
    } catch { if (current(captured, version)) publish({ phase: 'unavailable', notice: 'preview-unavailable' }); return }
    if (current(captured, version)) await readServer(false)
  }
  function choose(config: AvatarConfig) {
    if (!scope || !current(scope) || !preview || !state.draft || ['saving', 'loading', 'reconciling'].includes(state.phase)) return false
    try {
      const next = copyConfig(config)
      if (!preview.supports(next)) throw new Error('Unavailable')
      publish({ draft: next, notice: state.needsReconciliation ? state.notice : null }); void show(next); return true
    } catch { publish({ notice: 'invalid-selection' }); return false }
  }
  function commit(avatar: AvatarState, captured: PrivacyScope) {
    try { options.onCommitted?.(avatar, captured) } catch { if (current(captured)) publish({ notice: 'saved-refresh-needed' }) }
  }
  return {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    start() {
      if (started || disposed) return
      started = true
      removers = [options.auth.privacy.registerCleanup(() => { clear('unavailable'); publish({}) }), options.auth.privacy.subscribe(() => { void initialize() })]
      void initialize()
    },
    reload() { if (scope && current(scope) && !operation) return readServer(state.needsReconciliation || state.draft !== null); return Promise.resolve() },
    selectPreset(id: string) { const preset = state.catalog?.presets.find(p => p.id === id); if (!preset) { publish({ notice: 'invalid-selection' }); return false }; return choose(preset.config) },
    selectItem(layer: Layer, id: string | null) {
      if (!state.draft || !(id === null && layer === 'accessory') && !state.catalog?.items.some(item => item.layer === layer && item.id === id)) {
        publish({ notice: 'invalid-selection' }); return false
      }
      return choose({ ...state.draft, item_ids: { ...state.draft.item_ids, [layer]: id } })
    },
    selectTone(id: string) {
      if (!state.draft || !state.catalog?.tones.includes(id)) { publish({ notice: 'invalid-selection' }); return false }
      return choose({ ...state.draft, skin_tone_id: id })
    },
    canSelect(config: AvatarConfig) { try { return preview?.supports(copyConfig(config)) === true } catch { return false } },
    cancel() {
      if (state.phase === 'saving' || state.phase === 'reconciling' || !state.persisted || !scope || !current(scope)) return false
      const avatar = state.candidate ?? state.persisted
      publish({ persisted: avatar, draft: avatar.config, candidate: null, needsReconciliation: state.needsReconciliation && !state.candidate,
        notice: state.needsReconciliation && !state.candidate ? state.notice : null })
      if (state.needsReconciliation === false) commit(avatar, scope)
      void show(avatar.config); return true
    },
    resolveConflict(choice: 'keep-draft' | 'use-server') {
      if (!state.candidate || !scope || !current(scope) || operation) return false
      const avatar = state.candidate, draft = choice === 'use-server' ? avatar.config : state.draft
      publish({ persisted: avatar, draft, candidate: null, needsReconciliation: false, notice: null })
      commit(avatar, scope); if (draft) void show(draft); return true
    },
    async save() {
      if (!scope || !current(scope) || state.phase !== 'ready' || operation || state.needsReconciliation || !state.persisted || !state.draft
        || !preview?.supports(state.draft) || sameConfig(state.persisted.config, state.draft)) return false
      const captured = scope, version = epoch, submitted = copyConfig(state.draft), revision = state.persisted.revision
      const flight = new AbortController(); operation = flight; publish({ phase: 'saving', notice: null })
      try {
        const result = await options.auth.privacy.run(captured, ({ signal }) => options.auth.request({
          path: `/api/concert/events/${captured.eventId}/me/avatar`, method: 'PUT', access: 'protected', errorFamily: 'core', signal,
          body: { json: { expected_revision: revision, config: { ...submitted, item_ids: { ...submitted.item_ids } } } },
          decode: value => { const avatar = decodeAvatar(value); if (avatar.revision <= revision || !sameConfig(avatar.config, submitted)) throw new Error('Invalid save response'); return avatar },
        }), flight.signal)
        if (!current(captured, version) || flight.signal.aborted) return false
        publish({ phase: 'ready', persisted: result.data, draft: result.data.config, notice: 'saved', candidate: null })
        commit(result.data, captured); return true
      } catch (error) {
        if (current(captured, version) && !flight.signal.aborted) {
          const conflict = error instanceof TransportError && error.status === 409 && error.server?.reason === 'VERSION_CONFLICT'
          const uncertain = !(error instanceof TransportError) || error.uncertain
          publish({ phase: 'ready', needsReconciliation: conflict || uncertain, notice: conflict ? 'conflict' : uncertain ? 'uncertain' : 'save-failed' })
        }
        return false
      } finally { if (operation === flight) operation = null }
    },
    stop() { if (!started || disposed) return; started = false; removers.forEach(remove => remove()); removers = []; clear('idle'); publish({}) },
    dispose() { if (disposed) return; disposed = true; removers.forEach(remove => remove()); removers = []; clear('disposed'); publish({}); listeners.clear() },
  }
}
export type AvatarEditorController = ReturnType<typeof createAvatarEditorController>
