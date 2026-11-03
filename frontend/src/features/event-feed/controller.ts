import { blank, createFeedRuntime, failure, type FeedOptions, type Slot } from './runtime'
import { decoder, freeze, ordered, uuid, type Filter, type Notice, type Page, type Post } from './contract'

export interface FeedState {
  readonly filter: Filter; readonly available: boolean; readonly readOnly: boolean
  readonly feed: Slot<Page<Post>>; readonly notices: Slot<Page<Notice>>; readonly notice: Slot<Notice>
}
export function createEventFeedController(options: FeedOptions) {
  const initial = (): FeedState => ({ filter: 'ALL', available: false, readOnly: true, feed: blank(), notices: blank(), notice: blank() })
  let state = freeze(initial()), feedVersion = 0, noticesVersion = 0, noticeVersion = 0
  const listeners = new Set<() => void>(), seenCursors = { feed: new Set<string>(), notices: new Set<string>() }
  function publish(patch: Partial<FeedState>) { state = freeze({ ...state, ...patch }); listeners.forEach(fn => fn()) }
  const runtime = createFeedRuntime(options, () => {
    feedVersion++; noticesVersion++; noticeVersion++; seenCursors.feed.clear(); seenCursors.notices.clear(); state = freeze(initial()); listeners.forEach(fn => fn())
  }, () => { publish({ available: runtime.usable(), readOnly: runtime.access().readOnly }); if (runtime.usable()) { void loadFeed(); void loadNotices() } else publish({ feed: blank(), notices: blank(), notice: blank() }) })
  async function loadFeed(more = false) {
    if (!runtime.usable() || state.feed.phase === 'loading') return
    const previous = more ? state.feed.data : null, cursor = previous?.next_cursor
    if (more && !cursor) return
    const version = ++feedVersion, scope = runtime.scope()!, filter = state.filter
    publish({ feed: { phase: 'loading', data: previous, failure: null } })
    try {
      const data = await runtime.run('feed', s => ({ method: 'GET', path: `/api/concert/events/${s.eventId}/feed`, access: 'protected', errorFamily: 'domain',
        domainOperation: 'listConcertFeed', query: { limit: 20, filter, ...(cursor ? { cursor } : {}) },
        decode: decoder<Page<Post>>('Domain_feed_FeedPage', page => page.items.every(row => row.event_id === s.eventId
          && (filter !== 'WITH_MEDIA' || row.media_ids.length > 0) && (filter !== 'PARTY_LINK' || row.links.some(l => l.target_type === 'PARTY'))
          && (filter !== 'GIVEAWAY_LINK' || row.links.some(l => l.target_type === 'GIVEAWAY')))) }))
      if (version !== feedVersion || !options.auth.privacy.isCurrent(scope)) return
      const items = [...(previous?.items ?? []), ...data.items]
      if (!ordered(items, row => row.post_id) || (data.next_cursor && (data.next_cursor === cursor || seenCursors.feed.has(data.next_cursor)))) throw new Error('Invalid cursor/order')
      if (cursor) seenCursors.feed.add(cursor)
      if (!more) seenCursors.feed.clear()
      publish({ feed: { phase: 'ready', data: { items, next_cursor: data.next_cursor }, failure: null } })
    } catch (error) { if (version === feedVersion && options.auth.privacy.isCurrent(scope)) publish({ feed: { phase: 'error', data: null, failure: failure(error) } }) }
  }
  async function loadNotices(more = false) {
    if (!runtime.usable() || state.notices.phase === 'loading') return
    const previous = more ? state.notices.data : null, cursor = previous?.next_cursor
    if (more && !cursor) return
    const version = ++noticesVersion, scope = runtime.scope()!
    publish({ notices: { phase: 'loading', data: previous, failure: null } })
    try {
      const data = await runtime.run('notices', s => ({ method: 'GET', path: `/api/concert/events/${s.eventId}/notices`, access: 'protected', errorFamily: 'domain',
        domainOperation: 'listConcertNotices', query: { limit: 20, ...(cursor ? { cursor } : {}) },
        decode: decoder<Page<Notice>>('Domain_feed_PublicNoticeList', p => p.items.every(n => n.event_id === s.eventId)) }))
      if (version !== noticesVersion || !options.auth.privacy.isCurrent(scope)) return
      const items = [...(previous?.items ?? []), ...data.items]
      if (new Set(items.map(n => n.notice_id)).size !== items.length || (data.next_cursor && (data.next_cursor === cursor || seenCursors.notices.has(data.next_cursor)))) throw new Error('Invalid notices')
      if (cursor) seenCursors.notices.add(cursor)
      if (!more) seenCursors.notices.clear()
      publish({ notices: { phase: 'ready', data: { items, next_cursor: data.next_cursor }, failure: null } })
    } catch (error) { if (version === noticesVersion && options.auth.privacy.isCurrent(scope)) publish({ notices: { phase: 'error', data: null, failure: failure(error) } }) }
  }
  async function selectNotice(id: string) {
    if (!runtime.usable()) return
    const version = ++noticeVersion, scope = runtime.scope()!
    publish({ notice: { phase: 'loading', data: null, failure: null } })
    try {
      if (!uuid.test(id)) throw new Error('Invalid notice')
      const data = await runtime.run('notice', s => ({ method: 'GET', path: `/api/concert/events/${s.eventId}/notices/${id}`, access: 'protected', errorFamily: 'domain',
        domainOperation: 'getConcertNotice', decode: decoder<Notice>('Domain_feed_PublishedNotice', n => n.notice_id === id && n.event_id === s.eventId) }))
      if (version === noticeVersion && options.auth.privacy.isCurrent(scope)) publish({ notice: { phase: 'ready', data, failure: null } })
    } catch (error) {
      if (version === noticeVersion && options.auth.privacy.isCurrent(scope)) {
        // A withdrawn notice must disappear from both detail and its cached list.
        const data = state.notices.data
        publish({ notice: { phase: 'error', data: null, failure: failure(error) }, ...(data ? { notices: { ...state.notices, data: { ...data, items: data.items.filter(n => n.notice_id !== id) } } } : {}) })
      }
    }
  }
  return {
    read: () => state, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } }, start: runtime.start, dispose: runtime.dispose,
    loadFeed, loadNotices, selectNotice,
    setFilter(filter: Filter) {
      if (!['ALL', 'WITH_MEDIA', 'PARTY_LINK', 'GIVEAWAY_LINK'].includes(filter) || !runtime.usable()) return
      runtime.cancel('feed'); feedVersion++; seenCursors.feed.clear(); publish({ filter, feed: blank() }); void loadFeed()
    },
    /** Linked target denied/revoked callback: evict immediately; do not restore an old preview. */
    removeLink(target: { target_type: string; target_id: string }) {
      const data = state.feed.data
      if (data) publish({ feed: { ...state.feed, data: { ...data, items: data.items.map(post => ({ ...post, links: post.links.filter(l => l.target_type !== target.target_type || l.target_id !== target.target_id) })) } } })
    },
  }
}
export type EventFeedController = ReturnType<typeof createEventFeedController>
