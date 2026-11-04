import type { SessionController } from '../../auth/model/session'
import type { PrivacyScope } from '../../../shared/query/privacy'
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
  let flight: AbortController | null = null, revision = 0, disposed = false, refreshPending = false, avatarRevisionFloor = 0
  const listeners = new Set<() => void>()
  const publish = (patch: Partial<MeState>) => { state = freeze({ ...state, ...patch }); for (const f of Array.from(listeners)) { try { f() } catch { /* Observer isolation. */ } } }
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
    refreshPending = false
    const id = ++revision, controller = new AbortController(); flight = controller
    publish({ phase: 'loading', message: 'Loading your profile…', pending: null })
    try {
      const result = await auth.privacy.run(scope, async ({ signal }) => (await auth.request({ path: path(), method: 'GET', access: 'protected', errorFamily: 'core', signal,
        decode: value => { const p = decodeParticipant(value, scope); if ((state.participant && p.revision < state.participant.revision) || p.profile.avatar.revision < avatarRevisionFloor) throw new Error('Stale participant read'); return p } })).data, controller.signal)
      if (current(id)) {
        avatarRevisionFloor = Math.max(avatarRevisionFloor, result.profile.avatar.revision)
        publish({ phase: refreshPending ? 'conflict' : 'ready', participant: result, draft: draftOf(result), blocked: refreshPending, message: refreshPending ? 'Your avatar changed during loading. Reload the saved version before editing.' : 'Your profile is up to date.' })
      }
    } catch (error) { if (current(id)) { if (error instanceof TransportError && error.status === 403) invalidate(); else publish({ phase: 'error', blocked: state.blocked || avatarRevisionFloor > (state.participant?.profile.avatar.revision ?? 0), message: 'Could not load your profile. Retry to read the latest saved version.' }) } }
    finally { if (flight === controller) flight = null }
  }
  async function save(section: Section) {
    if (disposed || busy() || state.blocked || !state.participant || !dirty(section) || !auth.privacy.isCurrent(scope)) return
    const parsed = section === 'profile' ? profileSchema.safeParse({ display_name: state.draft.display_name, bio: state.draft.bio }) : preferencesSchema.safeParse(state.draft.preferences)
    if (!parsed.success) { publish({ message: section === 'profile'
      ? parsed.error.issues[0]?.message ?? 'Use a display name of 1–40 characters and a bio of at most 160 characters.'
      : 'Preferences must use the available choices.' }); return }
    const otherDirty = dirty(section === 'profile' ? 'preferences' : 'profile')
    const before = state.participant, draft = state.draft, id = ++revision, controller = new AbortController(); flight = controller
    const body = section === 'profile' ? { expected_revision: before.revision, ...profileSchema.parse(parsed.data) }
      : { expected_revision: before.revision, preferences: preferencesSchema.parse(parsed.data) }
    publish({ phase: 'saving', pending: section, message: 'Saving…' })
    try {
      const result = await auth.privacy.run(scope, async ({ signal }) => (await auth.request({ path: path(), method: 'PATCH', access: 'protected', errorFamily: 'core', signal,
        body: { json: body }, decode: value => {
          const p = decodeParticipant(value, scope)
          if (p.revision <= before.revision || p.profile.avatar.revision < avatarRevisionFloor) throw new Error('Stale participant revision')
          return p
        } })).data, controller.signal)
      if (current(id)) {
        avatarRevisionFloor = Math.max(avatarRevisionFloor, result.profile.avatar.revision)
        publish({ phase: refreshPending ? 'conflict' : 'ready', participant: result, pending: null, blocked: refreshPending,
        draft: !otherDirty ? draftOf(result) : section === 'profile' ? { ...draftOf(result), preferences: draft.preferences } : { ...draftOf(result), display_name: draft.display_name, bio: draft.bio },
        message: refreshPending ? 'Your avatar changed during saving. Reload the saved version before another save.' : section === 'profile' ? 'Profile saved.' : 'Preferences saved. Sound only starts after you explicitly apply it in this session.' })
      }
    } catch (error) {
      if (!current(id)) return
      if (error instanceof TransportError && error.status === 403) { invalidate(); return }
      const conflict = error instanceof TransportError && error.status === 409
      const uncertain = !(error instanceof TransportError) || error.uncertain || ['network', 'timeout', 'invalid-response', 'aborted'].includes(error.kind)
      publish({ phase: conflict ? 'conflict' : 'error', pending: null, blocked: conflict || uncertain || refreshPending,
        message: refreshPending ? 'Your avatar changed while saving. Your draft is kept. Reload the saved version before another save.' : conflict ? 'Your profile changed elsewhere. Your draft is kept. Discard it and reload before saving.'
          : uncertain ? 'The save result is unknown. Your draft is kept. Reload the saved version before another save.'
          : 'Could not save. Your draft is kept. Check your connection or permissions and try again.' })
    } finally { if (flight === controller) flight = null }
  }
  if (!auth.privacy.isCurrent(scope)) invalidate()
  return {
    scope,
    /** Delivery uses this after verified FE13 commits; never silently rebase a dirty draft. */
    async refreshAfterAvatar(expected: PrivacyScope, avatarRevision: number) {
      if (disposed || !auth.privacy.isCurrent(scope) || !auth.privacy.isCurrent(expected) || !Number.isSafeInteger(avatarRevision)
        || avatarRevision < 1 || avatarRevision <= avatarRevisionFloor || ['expired', 'disposed'].includes(state.phase)) return
      avatarRevisionFloor = avatarRevision
      if (busy()) { refreshPending = true; publish({ blocked: true, message: 'Your avatar changed. Waiting for the pending request before you can reload.' }); return }
      if (dirty('profile') || dirty('preferences')) publish({ phase: 'conflict', blocked: true, message: 'Your avatar changed. Your profile draft is kept. Discard drafts and reload the saved version before saving.' })
      else await load()
    },
    read: () => state, subscribe(f: () => void) { listeners.add(f); return () => { listeners.delete(f) } }, load, save, dirty,
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
