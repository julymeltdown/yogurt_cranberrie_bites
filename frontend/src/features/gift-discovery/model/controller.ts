import type { SessionController } from '../../auth/model/session'
import { projectCapabilities, type ShellCapabilities } from '../../../app/capabilities'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { TransportError } from '../../../shared/api/errors'
import { freezeGift, giftActivityDecoder, giftId, giveawayDecoder, giveawayListDecoder, validateFilters,
  type GiftActivity, type GiftFilters, type GiftQuery, type Giveaway, type Page, type Reservation } from './contract'

export interface GiftContext {
  /** The shell binds its current, validated bootstrap decisions to this FE02 lease. */
  readonly scope: PrivacyScope
  readonly capabilities: ShellCapabilities
  readonly eventPhase: 'DRAFT' | 'OPEN' | 'LIVE' | 'READ_ONLY' | 'ARCHIVED'
}
export interface GiftContextPort { read(): GiftContext | null; subscribe(listener: () => void): () => void }
export type GiftAuth = Pick<SessionController, 'session' | 'privacy' | 'request'>
export type GiftFailure = 'signed-out' | 'denied' | 'missing' | 'rate-limited' | 'invalid' | 'unavailable'
export interface GiftResource<T> {
  readonly phase: 'idle' | 'loading' | 'ready' | 'error'
  readonly data: T | null
  readonly failure: GiftFailure | null
  readonly retryAt: number | null
}
export interface GiftPage<T> extends GiftResource<Page<T>> { readonly page: number; readonly canPrevious: boolean }
export interface GiftDiscoveryState {
  readonly context: GiftContext | null
  readonly filters: GiftFilters
  readonly filterInvalid: boolean
  readonly list: GiftPage<Giveaway>
  readonly detail: GiftResource<Giveaway>
  readonly selectedId: string | null
  readonly giving: GiftPage<Giveaway>
  readonly receiving: GiftPage<Reservation>
  readonly now: number
  readonly disposed: boolean
}
export interface GiftClock { now(): number; setTimeout(callback: () => void, delay: number): unknown; clearTimeout(handle: unknown): void }
const defaultClock: GiftClock = { now: Date.now, setTimeout: (callback, delay) => globalThis.setTimeout(callback, delay),
  clearTimeout: handle => globalThis.clearTimeout(handle as ReturnType<typeof setTimeout>) }
const blank = <T>(): GiftResource<T> => ({ phase: 'idle', data: null, failure: null, retryAt: null })
const blankPage = <T>(): GiftPage<T> => ({ ...blank<Page<T>>(), page: 1, canPrevious: false })
type Lane = 'list' | 'detail' | 'activity'
type Collection = 'giving' | 'receiving'
type Trail = { cursor: string | null; previous: (string | null)[]; seen: Set<string>; cursors: Set<string>; boundary: { time: number; id: string } | null }
const trail = (): Trail => ({ cursor: null, previous: [], seen: new Set(), cursors: new Set(), boundary: null })
const errorView = (error: unknown, now: number): Pick<GiftResource<never>, 'failure' | 'retryAt'> => {
  if (!(error instanceof TransportError)) return { failure: 'invalid', retryAt: null }
  return { failure: error.status === 401 || error.kind === 'authentication-required' || error.kind === 'session-changed' ? 'signed-out'
    : error.status === 403 ? 'denied' : error.status === 404 ? 'missing' : error.status === 429 ? 'rate-limited'
      : error.kind === 'invalid-response' || error.kind === 'invalid-request' ? 'invalid' : 'unavailable',
  retryAt: error.status === 429 ? now + (error.retryAfterMs ?? 1000) : null }
}
export function giveawayStatus(gift: Giveaway, now: number): string {
  if (gift.state === 'DRAFT') return 'Draft'
  if (gift.state === 'CANCELLED') return 'Cancelled'
  if (gift.state === 'CLOSED') return 'Completed'
  if (now >= Date.parse(gift.pickup_ends_at)) return 'Ended'
  if (gift.state === 'PAUSED') return 'Paused'
  if (gift.available === 0) return 'Sold out'
  return now < Date.parse(gift.pickup_starts_at) ? 'Open · pickup later' : 'Open · pickup now'
}
/** Describes eligibility only. FE24 never implements a reservation/owner command. */
export function reservationEligibility(gift: Giveaway, state: GiftDiscoveryState) {
  const context = state.context
  if (!context || state.disposed) return 'Current access could not be confirmed.'
  for (const capability of ['gifts', 'mutations'] as const) {
    const decision = context.capabilities[capability]
    if (!decision.available) return `${capability === 'gifts' ? 'Gifts' : 'Changes'} unavailable: ${decision.reason}.`
  }
  if (context.eventPhase !== 'OPEN' && context.eventPhase !== 'LIVE') return 'This event is read only.'
  if (gift.owner.user_id === context.scope.userId) return 'You are the giver.'
  if (gift.state !== 'OPEN' || gift.available === 0 || state.now >= Date.parse(gift.pickup_ends_at)) {
    return `Reservations unavailable: ${giveawayStatus(gift, state.now).toLowerCase()}.`
  }
  if (state.receiving.data?.items.some(row => row.giveaway_id === gift.giveaway_id && (row.state === 'HELD' || row.state === 'READY'))) {
    return 'You already have a reservation. Check RECEIVING for its current status.'
  }
  // A page, even its terminal page, does not prove current authorization/nonmembership.
  return 'Your current reservation eligibility has not been confirmed.'
}

export function createGiftDiscoveryController(options: { auth: GiftAuth; context: GiftContextPort; clock?: GiftClock; limit?: number }) {
  const { auth } = options, clock = options.clock ?? defaultClock, limit = options.limit ?? 20
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) throw new Error('Invalid gift page limit')
  let state: GiftDiscoveryState = freezeGift({ context: null, filters: {}, filterInvalid: false, list: blankPage<Giveaway>(),
    detail: blank<Giveaway>(), selectedId: null, giving: blankPage<Giveaway>(), receiving: blankPage<Reservation>(), now: clock.now(), disposed: false })
  let started = false, disposed = false, removeCleanup = () => {}, removeContext = () => {}, timer: unknown = null
  let listTrail = trail(), givingTrail = trail(), receivingTrail = trail()
  const flights: Partial<Record<Lane, { id: number; abort: AbortController }>> = {}
  let revision = 0
  const listeners = new Set<() => void>()
  function publish(patch: Partial<GiftDiscoveryState>) {
    if (disposed) return
    state = freezeGift({ ...state, ...patch, now: clock.now() })
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Other owners must still receive synchronous eviction. */ } }
  }
  function cancel(lane?: Lane) {
    for (const key of (lane ? [lane] : ['list', 'detail', 'activity']) as Lane[]) { flights[key]?.abort.abort(); delete flights[key] }
  }
  function stopTimer() { if (timer !== null) clock.clearTimeout(timer); timer = null }
  function schedule() {
    stopTimer()
    if (disposed || !started || !state.context) return
    const now = clock.now(), dates = [
      ...(state.list.data?.items ?? []), ...(state.giving.data?.items ?? []), ...(state.detail.data ? [state.detail.data] : []),
    ].flatMap(row => [Date.parse(row.pickup_starts_at), Date.parse(row.pickup_ends_at)])
    dates.push(...(state.receiving.data?.items ?? []).map(row => Date.parse(row.expires_at)))
    if (state.list.retryAt) dates.push(state.list.retryAt)
    if (state.detail.retryAt) dates.push(state.detail.retryAt)
    if (state.giving.retryAt) dates.push(state.giving.retryAt)
    if (state.receiving.retryAt) dates.push(state.receiving.retryAt)
    const next = Math.min(...dates.filter(date => date > now))
    if (!Number.isFinite(next)) return
    timer = clock.setTimeout(() => { timer = null; publish({}); schedule() }, Math.min(next - now, 2_147_483_647))
  }
  function context(): GiftContext | null {
    try {
      const current = options.context.read()
      if (!current || !auth.privacy.isCurrent(current.scope) || !current.scope.eventId || !giftId.test(current.scope.eventId)
        || !current.scope.userId || !giftId.test(current.scope.userId)
        || !['DRAFT', 'OPEN', 'LIVE', 'READ_ONLY', 'ARCHIVED'].includes(current.eventPhase)) return null
      const capabilities = projectCapabilities(current.capabilities)
      if (['READ_ONLY', 'ARCHIVED'].includes(current.eventPhase) && capabilities.mutations.available) return null
      return freezeGift({ scope: { ...current.scope }, capabilities, eventPhase: current.eventPhase })
    } catch { return null }
  }
  function clear() {
    cancel(); stopTimer(); listTrail = trail(); givingTrail = trail(); receivingTrail = trail()
    publish({ context: null, filters: {}, filterInvalid: false, list: blankPage(), detail: blank(), selectedId: null, giving: blankPage(), receiving: blankPage() })
  }
  function qualified() {
    const current = context()
    return started && !disposed && state.context && current && JSON.stringify(current) === JSON.stringify(state.context)
      && current.capabilities.gifts.available ? current : null
  }
  function begin(lane: Lane) {
    cancel(lane)
    const captured = qualified()
    if (!captured) return null
    const flight = { id: ++revision, abort: new AbortController() }; flights[lane] = flight
    return { captured, flight, current: () => !disposed && flights[lane] === flight && !flight.abort.signal.aborted
      && auth.privacy.isCurrent(captured.scope) && qualified() !== null }
  }
  function checkPage<T extends { readonly created_at: string }>(page: Page<T>, history: Trail, id: (row: T) => string) {
    if (page.next_cursor && (history.cursors.has(page.next_cursor) || history.previous.includes(page.next_cursor))) throw new Error('Cursor cycle')
    if (page.items.some(row => history.seen.has(id(row)))) throw new Error('Duplicate page item')
    const first = page.items[0], boundary = history.boundary
    if (first && boundary && !(Date.parse(first.created_at) < boundary.time
      || (Date.parse(first.created_at) === boundary.time && id(first) < boundary.id))) throw new Error('Invalid page order')
  }
  function pageState<T>(page: Page<T>, history: Trail): GiftPage<T> {
    return { phase: 'ready', data: page, failure: null, retryAt: null, page: history.previous.length + 1, canPrevious: history.previous.length > 0 }
  }
  async function list() {
    if (state.list.retryAt !== null && clock.now() < state.list.retryAt) return
    const work = begin('list'); if (!work) return
    const { captured, flight, current } = work
    const query: GiftQuery = { limit, ...state.filters, ...(listTrail.cursor ? { cursor: listTrail.cursor } : {}) }
    publish({ list: { ...blankPage(), phase: 'loading', page: listTrail.previous.length + 1, canPrevious: listTrail.previous.length > 0 } })
    try {
      const result = await auth.privacy.run(captured.scope, ({ signal }) => auth.request({
        path: `/api/concert/events/${captured.scope.eventId}/giveaways`, method: 'GET', access: 'protected', errorFamily: 'domain',
        domainOperation: 'listConcertGiveaways', query, decode: giveawayListDecoder(captured.scope, query), signal,
      }), flight.abort.signal)
      if (!current()) return
      checkPage(result.data, listTrail, row => row.giveaway_id)
      publish({ list: pageState(result.data, listTrail) })
    } catch (error) { if (current()) publish({ list: { ...state.list, phase: 'error', data: null, ...errorView(error, clock.now()) } }) }
    finally { if (current()) { delete flights.list; schedule() } }
  }
  async function detail() {
    if (state.detail.retryAt !== null && clock.now() < state.detail.retryAt) return
    const id = state.selectedId
    if (!id) return
    if (!giftId.test(id)) { publish({ detail: { ...blank(), phase: 'error', failure: 'missing' } }); return }
    const work = begin('detail'); if (!work) return
    const { captured, flight, current } = work
    publish({ detail: { ...blank(), phase: 'loading' } })
    try {
      const result = await auth.privacy.run(captured.scope, ({ signal }) => auth.request({
        path: `/api/concert/events/${captured.scope.eventId}/giveaways/${id}`, method: 'GET', access: 'protected', errorFamily: 'domain',
        domainOperation: 'getConcertGiveaway', decode: giveawayDecoder(captured.scope, id), signal,
      }), flight.abort.signal)
      if (current() && state.selectedId === id) publish({ detail: { ...blank(), phase: 'ready', data: result.data } })
    } catch (error) { if (current()) publish({ detail: { ...blank(), phase: 'error', ...errorView(error, clock.now()) } }) }
    finally { if (current()) { delete flights.detail; schedule() } }
  }
  async function activity(only?: Collection) {
    if ([state.giving.retryAt, state.receiving.retryAt].some(at => at !== null && clock.now() < at)) return
    const work = begin('activity'); if (!work) return
    const { captured, flight, current } = work
    const query: GiftQuery = { limit, ...(givingTrail.cursor ? { giving_cursor: givingTrail.cursor } : {}),
      ...(receivingTrail.cursor ? { receiving_cursor: receivingTrail.cursor } : {}) }
    const loading = <T>(page: GiftPage<T>): GiftPage<T> => ({ ...page, phase: 'loading', data: null, failure: null, retryAt: null })
    publish({ ...(!only || only === 'giving' ? { giving: loading(state.giving) } : {}), ...(!only || only === 'receiving' ? { receiving: loading(state.receiving) } : {}) })
    try {
      const result = await auth.privacy.run(captured.scope, ({ signal }) => auth.request({
        path: `/api/concert/events/${captured.scope.eventId}/me/giveaways`, method: 'GET', access: 'protected', errorFamily: 'domain',
        domainOperation: 'getConcertGiftActivity', query, decode: giftActivityDecoder(captured.scope, query), signal,
      }), flight.abort.signal)
      if (!current()) return
      const value: GiftActivity = result.data
      if (!only || only === 'giving') checkPage(value.giving, givingTrail, row => row.giveaway_id)
      if (!only || only === 'receiving') checkPage(value.receiving, receivingTrail, row => row.reservation_id)
      publish({ ...(!only || only === 'giving' ? { giving: pageState(value.giving, givingTrail) } : {}),
        ...(!only || only === 'receiving' ? { receiving: pageState(value.receiving, receivingTrail) } : {}) })
    } catch (error) {
      if (current()) {
        const failed = <T>(page: GiftPage<T>): GiftPage<T> => ({ ...page, phase: 'error', data: null, ...errorView(error, clock.now()) })
        publish({ ...(!only || only === 'giving' ? { giving: failed(state.giving) } : {}), ...(!only || only === 'receiving' ? { receiving: failed(state.receiving) } : {}) })
      }
    } finally { if (current()) { delete flights.activity; schedule() } }
  }
  function syncContext() {
    if (disposed || !started) return
    const next = context()
    if (JSON.stringify(next) === JSON.stringify(state.context)) return
    const selectedId = state.selectedId
    clear(); publish({ context: next, selectedId })
    if (next?.capabilities.gifts.available) { void list(); if (selectedId) void detail() }
  }
  function turn<T extends { readonly created_at: string }>(history: Trail, page: GiftPage<T>, direction: 'next' | 'previous', id: (row: T) => string): boolean {
    if (page.phase !== 'ready' || !page.data) return false
    if (direction === 'previous') {
      if (!history.previous.length) return false
      history.cursor = history.previous.pop()!; history.seen.clear(); history.cursors.clear(); history.boundary = null; return true
    }
    if (!page.data.next_cursor || history.previous.length >= 19) return false
    page.data.items.forEach(row => history.seen.add(id(row)))
    const last = page.data.items.at(-1)
    if (last) history.boundary = { time: Date.parse(last.created_at), id: id(last) }
    if (history.cursor) history.cursors.add(history.cursor)
    history.previous.push(history.cursor); history.cursor = page.data.next_cursor
    return true
  }
  return {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    start() {
      if (started || disposed) return
      started = true; removeCleanup = auth.privacy.registerCleanup(clear); removeContext = options.context.subscribe(syncContext); syncContext()
    },
    setFilters(input: GiftFilters) {
      if (!qualified()) return false
      try { const filters = validateFilters(input); cancel('list'); listTrail = trail(); publish({ filters, filterInvalid: false }); void list(); return true }
      catch { publish({ filterInvalid: true }); return false }
    },
    select(id: string | null) {
      if (disposed || (id === state.selectedId && state.detail.phase !== 'idle')) return
      cancel('detail'); publish({ selectedId: id, detail: blank() }); if (id) void detail(); else schedule()
    },
    loadActivity: () => activity(),
    retryList: () => list(), retryDetail: () => detail(),
    retryActivity: (collection: Collection) => activity(collection),
    pageList(direction: 'next' | 'previous') { if (!qualified() || !turn(listTrail, state.list, direction, row => row.giveaway_id)) return false; void list(); return true },
    pageActivity(collection: Collection, direction: 'next' | 'previous') {
      if (!qualified() || flights.activity) return false
      const changed = collection === 'giving' ? turn(givingTrail, state.giving, direction, row => row.giveaway_id)
        : turn(receivingTrail, state.receiving, direction, row => row.reservation_id)
      if (!changed) return false
      void activity(collection); return true
    },
    dispose() {
      if (disposed) return
      clear(); removeCleanup(); removeContext(); started = false; disposed = true
      state = freezeGift({ ...state, disposed: true })
      for (const listener of Array.from(listeners)) { try { listener() } catch { /* Final eviction. */ } }
      listeners.clear()
    },
  }
}
export type GiftDiscoveryController = ReturnType<typeof createGiftDiscoveryController>
