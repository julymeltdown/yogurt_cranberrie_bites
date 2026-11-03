import { TransportError } from '../../shared/api/errors'
import { blank, createSocialRuntime, failure, type Slot, type SocialOptions } from './runtime'
import { decodePage, decodeProfile, freeze, ordered, querySchema, uuid, type IntroPage, type PublicProfile } from './contract'
export interface PeopleState { query: string; input: string; available: boolean; page: Slot<IntroPage>; profile: Slot<PublicProfile>; target: string | null }
export function createPeopleController(options: SocialOptions) {
  const initial = (): PeopleState => ({ query: '', input: '', available: false, page: blank(), profile: blank(), target: null })
  let state = freeze(initial()), pageVersion = 0, profileVersion = 0
  const listeners = new Set<() => void>(), cursors = new Set<string>(), hiddenAuthors = new Set<string>()
  function publish(patch: Partial<PeopleState>) { state = freeze({ ...state, ...patch }); [...listeners].forEach(fn => { try { fn() } catch { /* Isolate observers so privacy cleanup reaches every subscriber. */ } }) }
  const runtime = createSocialRuntime(options, () => { pageVersion++; profileVersion++; cursors.clear(); hiddenAuthors.clear(); state = freeze(initial()); [...listeners].forEach(fn => { try { fn() } catch { /* Isolate observers so privacy cleanup reaches every subscriber. */ } }) },
    () => { if (!runtime.usable()) { pageVersion++; profileVersion++; cursors.clear(); publish({ page: blank(), profile: blank() }) }; publish({ available: runtime.usable() }); if (runtime.usable() && state.page.phase === 'idle') void load() })
  async function load(more = false) {
    if (!runtime.usable() || state.page.phase === 'loading') return
    const previous = more ? state.page.data : null, cursor = previous?.next_cursor, captured = runtime.scope()!, version = ++pageVersion, query = state.query
    if (more && !cursor) return
    publish({ page: { phase: 'loading', data: previous, failure: null } })
    try {
      const page = await runtime.run('people', scope => ({ method: 'GET', path: `/api/concert/events/${scope.eventId}/intros`, access: 'protected', errorFamily: 'domain', domainOperation: 'listConcertIntros',
        query: { limit: 20, ...(query ? { q: query } : {}), ...(cursor ? { cursor } : {}) }, decode: value => decodePage(value, scope) }))
      if (version !== pageVersion || !runtime.current(captured)) return
      const items = [...(previous?.items ?? []), ...page.items]
      if (!ordered(items) || new Set(items.map(row => row.intro_id)).size !== items.length || (page.next_cursor && (page.next_cursor === cursor || cursors.has(page.next_cursor)))) throw new Error('Invalid pagination')
      if (cursor) cursors.add(cursor)
      publish({ page: { phase: 'ready', data: { items: items.filter(row => !hiddenAuthors.has(row.author.user_id)), next_cursor: page.next_cursor }, failure: null } })
    } catch (error) { if (version === pageVersion && runtime.current(captured)) publish({ page: { phase: 'error', data: null, failure: failure(error) } }) }
  }
  async function openProfile(id: string) {
    if (!runtime.usable()) return
    const captured = runtime.scope()!, version = ++profileVersion
    publish({ profile: { phase: 'loading', data: null, failure: null }, target: id })
    try {
      uuid.parse(id)
      const data = await runtime.run('profile', scope => ({ path: `/api/concert/events/${scope.eventId}/people/${id}`, method: 'GET', access: 'protected', errorFamily: 'core', decode: value => decodeProfile(value, id) }))
      if (version === profileVersion && runtime.current(captured)) { hiddenAuthors.delete(id); publish({ profile: { phase: 'ready', data, failure: null } }) }
    } catch (error) {
      if (version !== profileVersion || !runtime.current(captured)) return
      if (error instanceof TransportError && [403, 404].includes(error.status ?? 0)) hiddenAuthors.add(id)
      pageVersion++; runtime.cancel('people')
      const page = state.page.data
      // Direct-ID denied/hidden paths erase the same author's cached discovery rows.
      publish({ profile: { phase: 'error', data: null, failure: failure(error) }, ...(page ? { page: { phase: 'ready', failure: null, data: { ...page, items: page.items.filter(row => row.author.user_id !== id) } } } : { page: { phase: 'ready', data: { items: [], next_cursor: null }, failure: null } }) })
    }
  }
  return { read: () => state, subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } }, start: runtime.start, dispose: runtime.dispose, load, openProfile,
    setSearch(value: string) { if (runtime.usable()) publish({ input: value }) },
    setQuery(query: string) { const value = querySchema.safeParse(query); if (!value.success || !runtime.usable()) return false
      pageVersion++; runtime.cancel('people'); cursors.clear(); publish({ query: value.data, input: value.data, page: blank() }); void load(); return true },
    closeProfile() { profileVersion++; runtime.cancel('profile'); publish({ profile: blank(), target: null }) },
  }
}
export type PeopleController = ReturnType<typeof createPeopleController>
