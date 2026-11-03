import type { SessionController } from '../../auth/model/session'
import { TransportError } from '../../../shared/api/errors'
import { decodeParticipant, defaults, draftOf, freeze, preferencesSchema, profileSchema, type Draft, type Participant, type Preferences, type ProfileDraft } from './contract'

export type Section = 'profile' | 'preferences'
export interface MeState {
  readonly phase: 'idle' | 'loading' | 'ready' | 'saving' | 'conflict' | 'error' | 'expired' | 'disposed'
  readonly participant: Participant | null
  readonly draft: Draft
  readonly message: string
  readonly blocked: boolean
  readonly pending: Section | null
}
const blank = (): Draft => freeze({ display_name: '', bio: null, preferences: { ...defaults } })
/** Shell supplies its canonical controller. One instance per captured identity/event/policy lease. */
export function createMeController(auth: Pick<SessionController, 'request' | 'privacy'>) {
  const scope = auth.privacy.capture()
  let state: MeState = freeze({ phase: 'idle', participant: null, draft: blank(), message: '', blocked: false, pending: null })
  let flight: AbortController | null = null, revision = 0, disposed = false
  const listeners = new Set<() => void>()
  const publish = (patch: Partial<MeState>) => { state = freeze({ ...state, ...patch }); for (const f of [...listeners]) { try { f() } catch { /* Observer isolation. */ } } }
  const current = (id: number) => !disposed && revision === id && auth.privacy.isCurrent(scope)
  const invalidate = () => { ++revision; flight?.abort(); flight = null; publish({ phase: 'expired', participant: null, draft: blank(), pending: null, blocked: true, message: 'Your session or privacy permissions changed. Reopen ME to continue.' }) }
  const unsubscribe = auth.privacy.registerCleanup(invalidate)
  const path = () => {
    if (!scope.eventId || !scope.userId || !auth.privacy.isCurrent(scope)) throw new Error('Unavailable session')
    return `/api/concert/events/${scope.eventId}/me`
  }
  const busy = () => ['loading', 'saving', 'expired', 'disposed'].includes(state.phase)
  const dirty = (section: Section) => state.participant !== null && (section === 'profile'
    ? state.draft.display_name !== state.participant.profile.display_name || state.draft.bio !== state.participant.profile.bio
    : JSON.stringify(state.draft.preferences) !== JSON.stringify(state.participant.preferences))
  async function load() {
    if (disposed || busy() || !auth.privacy.isCurrent(scope)) return
    const id = ++revision, controller = new AbortController(); flight = controller
    publish({ phase: 'loading', message: 'Loading your profile…', pending: null })
    try {
      const result = await auth.privacy.run(scope, async ({ signal }) => (await auth.request({ path: path(), method: 'GET', access: 'protected', errorFamily: 'core', signal,
        decode: value => decodeParticipant(value, scope) })).data, controller.signal)
      if (current(id)) publish({ phase: 'ready', participant: result, draft: draftOf(result), blocked: false, message: 'Your profile is up to date.' })
    } catch { if (current(id)) publish({ phase: 'error', message: 'Could not load your profile. Retry to read the latest saved version.' }) }
    finally { if (flight === controller) flight = null }
  }
  async function save(section: Section) {
    if (disposed || busy() || state.blocked || !state.participant || !dirty(section) || !auth.privacy.isCurrent(scope)) return
    const parsed = section === 'profile' ? profileSchema.safeParse({ display_name: state.draft.display_name, bio: state.draft.bio }) : preferencesSchema.safeParse(state.draft.preferences)
    if (!parsed.success) { publish({ message: 'Use a display name of 1–40 characters and a bio of at most 160 characters. Preferences must use the available choices.' }); return }
    const before = state.participant, draft = state.draft, id = ++revision, controller = new AbortController(); flight = controller
    const body = section === 'profile' ? { expected_revision: before.revision, ...profileSchema.parse(parsed.data) }
      : { expected_revision: before.revision, preferences: preferencesSchema.parse(parsed.data) }
    publish({ phase: 'saving', pending: section, message: 'Saving…' })
    try {
      const result = await auth.privacy.run(scope, async ({ signal }) => (await auth.request({ path: path(), method: 'PATCH', access: 'protected', errorFamily: 'core', signal,
        body: { json: body }, decode: value => {
          const p = decodeParticipant(value, scope)
          if (p.revision <= before.revision) throw new Error('Stale participant revision')
          return p
        } })).data, controller.signal)
      if (current(id)) publish({ phase: 'ready', participant: result, pending: null, blocked: false,
        draft: section === 'profile' ? { ...draftOf(result), preferences: draft.preferences } : { ...draftOf(result), display_name: draft.display_name, bio: draft.bio },
        message: section === 'profile' ? 'Profile saved.' : 'Preferences saved. Sound only starts after you explicitly apply it in this session.' })
    } catch (error) {
      if (!current(id)) return
      const conflict = error instanceof TransportError && error.status === 409
      const uncertain = !(error instanceof TransportError) || error.uncertain || ['network', 'timeout', 'invalid-response', 'aborted'].includes(error.kind)
      publish({ phase: conflict ? 'conflict' : 'error', pending: null, blocked: conflict || uncertain,
        message: conflict ? 'Your profile changed elsewhere. Your draft is kept. Discard it and reload before saving.'
          : uncertain ? 'The save result is unknown. Your draft is kept. Reload the saved version before another save.'
          : 'Could not save. Your draft is kept. Check your connection or permissions and try again.' })
    } finally { if (flight === controller) flight = null }
  }
  if (!auth.privacy.isCurrent(scope)) invalidate()
  return {
    scope, read: () => state, subscribe(f: () => void) { listeners.add(f); return () => { listeners.delete(f) } }, load, save, dirty,
    editProfile(patch: Partial<ProfileDraft>) { if (!disposed && !busy() && state.participant) publish({ draft: { ...state.draft, ...patch }, message: 'Unsaved profile changes.' }) },
    editPreferences(patch: Partial<Preferences>) { if (!disposed && !busy() && state.participant) publish({ draft: { ...state.draft, preferences: { ...state.draft.preferences, ...patch } }, message: 'Unsaved preference changes.' }) },
    cancel(section: Section) {
      if (disposed || busy() || !state.participant) return
      const saved = draftOf(state.participant)
      publish({ draft: section === 'profile' ? { ...state.draft, display_name: saved.display_name, bio: saved.bio } : { ...state.draft, preferences: saved.preferences },
        message: state.blocked ? 'Draft discarded locally. Reload before saving again.' : 'Unsaved changes discarded.' })
    },
    dispose() { if (disposed) return; disposed = true; ++revision; flight?.abort(); flight = null; unsubscribe(); publish({ phase: 'disposed', participant: null, draft: blank(), pending: null, blocked: true, message: '' }); listeners.clear() },
  }
}
export type MeController = ReturnType<typeof createMeController>
