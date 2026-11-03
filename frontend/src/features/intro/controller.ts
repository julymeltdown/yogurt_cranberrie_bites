import { blank, createSocialRuntime, failure, type Slot, type SocialOptions } from '../people/runtime'
import { freeze } from '../people/contract'
import { TransportError } from '../../shared/api/errors'
import { createCommand, idleCommand, type CommandState } from './command'
import { decodeOwn, draftSchema, emptyDraft, fromIntro, sameDraft, type IntroDraft, type OwnIntro } from './contract'
export interface IntroState { draft: IntroDraft; saved: OwnIntro | null; latest: OwnIntro | null; load: Slot<OwnIntro>; command: CommandState; available: boolean; readOnly: boolean; needsReview: boolean; message: string }
export function createIntroController(options: SocialOptions) {
  const initial = (): IntroState => ({ draft: emptyDraft(), saved: null, latest: null, load: blank(), command: idleCommand(), available: false, readOnly: true, needsReview: false, message: '' })
  let state = freeze(initial()), version = 0, initialized = false
  const listeners = new Set<() => void>(), now = options.now ?? Date.now
  const emit = (patch: Partial<IntroState>) => { state = freeze({ ...state, ...patch }); [...listeners].forEach(fn => { try { fn() } catch { /* Isolate observers so privacy cleanup reaches every subscriber. */ } }) }
  const runtime = createSocialRuntime(options, () => { version++; initialized = false; state = freeze(initial()); command.reset(); }, () => { if (!runtime.usable()) { version++; initialized = false; command.interrupt(); emit({ load: blank() }) }; emit({ available: runtime.usable(), readOnly: runtime.readOnly() }); if (runtime.usable() && !initialized) { initialized = true; void load() } })
  async function fetchOwn() { return runtime.run('own-intro', scope => ({ path: `/api/concert/events/${scope.eventId}/me/intro`, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getOwnConcertIntro', decode: value => decodeOwn(value, scope) })) }
  function accept(row: OwnIntro) { const preserve = state.saved && !sameDraft(state.draft, fromIntro(state.saved)) && !sameDraft(state.draft, fromIntro(row)); emit({ saved: row, draft: preserve ? state.draft : fromIntro(row), latest: null, needsReview: false, load: { phase: 'ready', data: row, failure: null }, message: preserve ? 'Server change confirmed. Your unsaved draft is preserved.' : '' }) }
  const command = createCommand(runtime, options, c => emit({ command: c, ...(c.phase === 'conflict' ? { needsReview: true } : {}) }), accept, async (id, minimum) => {
    const row = await fetchOwn(); if (row.intro_id !== id || row.revision < minimum) throw new Error('Stale intro read'); return row
  })
  async function load() {
    if (!runtime.usable() || state.load.phase === 'loading' || state.command.locked) return
    const captured = runtime.scope()!, attempt = ++version
    emit({ load: { phase: 'loading', data: null, failure: null } })
    try {
      const row = await fetchOwn(); if (!runtime.current(captured) || attempt !== version) return
      if (state.saved && (row.revision < state.saved.revision || row.intro_id !== state.saved.intro_id || row.revision === state.saved.revision && JSON.stringify(row) !== JSON.stringify(state.saved))) throw new Error('Stale own intro')
      if (state.saved !== null || !sameDraft(state.draft, emptyDraft())) emit({ latest: row, needsReview: true, load: { phase: 'ready', data: row, failure: null }, message: 'Latest introduction loaded. Review it before keeping your draft or using the saved version.' })
      else accept(row)
    } catch (error) {
      if (!runtime.current(captured) || attempt !== version) return
      // A 404 is opaque, not proof that expected_revision=0 creation is safe.
      emit({ load: { phase: 'error', data: null, failure: failure(error) }, message: error instanceof TransportError && error.status === 404 ? 'Your introduction is unavailable. You can prepare a draft; the server decides whether creation is permitted.' : 'The introduction could not be loaded. Your draft is preserved.' })
    }
  }
  function validDraft() { const result = draftSchema.safeParse(state.draft); return result.success && (result.data.expires_at === null || Date.parse(result.data.expires_at) > now()) ? result.data : null }
  return { read: () => state, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } }, start: runtime.start, load, checkStatus: command.check,
    edit(draft: IntroDraft) { if (!runtime.usable() || state.command.locked) return; emit({ draft: structuredClone(draft), message: '' }) },
    cancel() { if (!runtime.usable() || state.command.locked) return; emit({ draft: state.saved ? fromIntro(state.saved) : emptyDraft(), message: 'Unsaved changes cancelled.' }) },
    reconcile(choice: 'keep-draft' | 'use-server') { if (!runtime.usable() || !state.latest || state.command.locked) return; const latest = state.latest
      emit({ saved: latest, draft: choice === 'use-server' ? fromIntro(latest) : state.draft, latest: null, needsReview: false, message: 'Latest revision selected. Save explicitly to apply any draft.' }) },
    save() {
      const draft = validDraft(); if (!draft || state.needsReview || !runtime.writable() || state.command.locked || (state.saved?.state === 'PUBLISHED' && draft.text.trim() === '')) { emit({ message: 'Check your text, complete half-open bounds, expiry and event access before saving.' }); return Promise.resolve(false) }
      const captured = runtime.scope()!, previous = state.saved, expected = previous?.revision ?? 0
      return command.submit({ operation: 'setConcertIntro', path: `/api/concert/events/${captured.eventId}/me/intro`, method: 'PUT', target: previous?.intro_id ?? null,
        json: { expected_revision: expected, text: draft.text, age_bounds: { ...draft.age_bounds }, expires_at: draft.expires_at }, decode(value) { const row = decodeOwn(value, captured)
          if (row.state !== (previous?.state === 'PUBLISHED' ? 'PUBLISHED' : 'DRAFT') || row.revision !== expected + 1 || (previous && row.intro_id !== previous.intro_id) || !sameDraft(fromIntro(row), draft)) throw new Error('Invalid intro save'); return row } })
    },
    transition(action: 'publish' | 'close') {
      const row = state.saved; if (!runtime.usable() || !row || state.needsReview || state.command.locked || action === 'publish' && (!runtime.writable() || row.state !== 'DRAFT' || !row.text.trim() || !sameDraft(state.draft, fromIntro(row))) || action === 'close' && row.state === 'CLOSED') return Promise.resolve(false)
      const captured = runtime.scope()!
      return command.submit({ operation: action === 'publish' ? 'publishConcertIntro' : 'closeConcertIntro', path: `/api/concert/events/${captured.eventId}/me/intro/${action}`, method: 'POST', target: row.intro_id, json: { expected_revision: row.revision }, safety: action === 'close',
        decode(value) { const next = decodeOwn(value, captured); if (next.intro_id !== row.intro_id || next.revision !== row.revision + 1 || next.state !== (action === 'publish' ? 'PUBLISHED' : 'CLOSED')) throw new Error('Invalid intro transition'); return next } })
    },
    dispose() { runtime.dispose() },
  }
}
export type IntroController = ReturnType<typeof createIntroController>
