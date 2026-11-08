import type { SessionController } from '../auth/model/session'
import type { HttpClient, HttpRequest } from '../../shared/api/http'
import { TransportError } from '../../shared/api/errors'
import type { PrivacyScope } from '../../shared/query/privacy'
import {
  addFavorite,
  canAdd,
  CATEGORIES,
  CATEGORY_PATH,
  emptySelection,
  isSelected,
  itemsFor,
  MAX_PER_CATEGORY,
  removeFavorite,
  validQuery,
  validateSelection,
  decodeCatalogPage,
  type CatalogItem,
  type Category,
  type FavoriteItem,
  type FavoriteSelection,
} from './contract'

/**
 * WP-03 T-804: the favorite picker controller.
 *
 * The controller owns three things and nothing else:
 *
 * * which tab is open (music, then anime, then games — the product's order);
 * * the current search term and its bounded result page;
 * * the current selection, capped at five per category.
 *
 * It deliberately has no intensity, importance or preference control, no
 * score, and no ranking. The donor's panel had all of those; the destination
 * does not.
 *
 * Skipping every category is a first-class outcome. `selection` starts empty,
 * `canSave` is true for an empty selection, and nothing in this controller
 * blocks on a non-empty pick. That is what makes "skipping all categories
 * leaves the profile fully usable" true on the client as well as on the server.
 */

export interface FavoritesOptions {
  readonly auth: Pick<SessionController, 'privacy'>
  readonly request: HttpClient['request']
}

export type Failure = 'unavailable' | 'invalid' | 'offline' | 'rate-limited'

export interface Slot<T> {
  readonly phase: 'idle' | 'loading' | 'ready' | 'error'
  readonly data: T | null
  readonly failure: Failure | null
}

export interface FavoritesState {
  readonly category: Category
  readonly term: string
  readonly results: Slot<readonly CatalogItem[]>
  readonly nextCursor: string | null
  readonly selection: FavoriteSelection
  readonly message: string
  readonly available: boolean
}

const blank = <T>(): Slot<T> => ({ phase: 'idle', data: null, failure: null })

function failure(error: unknown): Failure {
  if (!(error instanceof TransportError)) return 'invalid'
  if (error.status === 429) return 'rate-limited'
  if (error.kind === 'network' || error.kind === 'timeout') return 'offline'
  if (error.status === 400) return 'invalid'
  return 'unavailable'
}

const failureMessage: Readonly<Record<Failure, string>> = Object.freeze({
  unavailable: 'The catalog is unavailable. Try again in a moment.',
  invalid: 'That search could not be sent. Check the term and try again.',
  offline: 'Connection unavailable. Retry explicitly.',
  'rate-limited': 'Too many searches. Wait before trying again.',
})

export function createFavoritesController(options: FavoritesOptions) {
  let state: FavoritesState = Object.freeze({
    category: 'MUSIC',
    term: '',
    results: blank<readonly CatalogItem[]>(),
    nextCursor: null,
    selection: emptySelection(),
    message: '',
    available: false,
  })
  let disposed = false
  let started = false
  let epoch = 0
  const flights = new Map<string, AbortController>()
  const listeners = new Set<() => void>()

  const emit = (patch: Partial<FavoritesState>) => {
    state = Object.freeze({ ...state, ...patch })
    for (const listener of [...listeners]) {
      try {
        listener()
      } catch {
        // Isolate observers so one bad subscriber cannot stop the others.
      }
    }
  }

  const scope = (): PrivacyScope | null => {
    const captured = options.auth.privacy.capture()
    return options.auth.privacy.isCurrent(captured) ? captured : null
  }

  const usable = () => started && !disposed && scope() !== null

  function cancel(lane: string) {
    flights.get(lane)?.abort()
    flights.delete(lane)
  }

  async function run<T>(lane: string, build: () => HttpRequest<T>): Promise<T> {
    const captured = scope()
    if (!captured) throw new TransportError('authentication-required')
    cancel(lane)
    const flight = new AbortController()
    const version = epoch
    flights.set(lane, flight)
    try {
      const request = build()
      const result = await options.request({ ...request, signal: flight.signal })
      if (version !== epoch || !options.auth.privacy.isCurrent(captured)) {
        throw new TransportError('session-changed')
      }
      return result.data
    } finally {
      if (flights.get(lane) === flight) flights.delete(lane)
    }
  }

  async function search(category: Category, term: string, cursor: string | null): Promise<void> {
    const query = validQuery(term)
    if (query === null) {
      emit({ results: { phase: 'idle', data: null, failure: null }, nextCursor: null })
      return
    }
    emit({
      results: { phase: 'loading', data: cursor === null ? null : state.results.data, failure: null },
      message: '',
    })
    try {
      const page = await run(`catalog:${category}`, () => ({
        path: `/api/catalog/${CATEGORY_PATH[category]}/search`,
        method: 'GET',
        access: 'protected',
        errorFamily: 'core',
        query: { q: query, limit: 20, ...(cursor === null ? {} : { cursor }) },
        decode: (value: unknown) => decodeCatalogPage(value, category),
      }))
      if (state.category !== category) return
      const previous = cursor === null ? [] : state.results.data ?? []
      const merged = [...previous, ...page.items]
      emit({
        results: { phase: 'ready', data: merged, failure: null },
        nextCursor: page.next_cursor,
      })
    } catch (error) {
      if (state.category !== category) return
      const reason = failure(error)
      emit({
        results: { phase: 'error', data: null, failure: reason },
        nextCursor: null,
        message: failureMessage[reason],
      })
    }
  }

  return {
    read: () => state,
    subscribe(listener: () => void) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    start() {
      if (started || disposed) return
      started = true
      emit({ available: usable() })
    },
    /** Switch tab. The order of the tabs is the product's, not the caller's. */
    selectCategory(category: Category) {
      if (!CATEGORIES.includes(category) || state.category === category) return
      cancel(`catalog:${state.category}`)
      emit({
        category,
        term: '',
        results: blank<readonly CatalogItem[]>(),
        nextCursor: null,
        message: '',
      })
    },
    /** The tabs, in order: music, anime, games. */
    tabs: () => CATEGORIES,
    setTerm(term: string) {
      emit({ term })
    },
    /** Run the search for the current term. A blank term clears the results. */
    submit(): Promise<void> {
      if (!usable()) return Promise.resolve()
      return search(state.category, state.term, null)
    },
    more(): Promise<void> {
      if (!usable() || state.nextCursor === null) return Promise.resolve()
      return search(state.category, state.term, state.nextCursor)
    },
    /**
     * Add an item to its own category. A sixth item in one category is refused
     * here, before any request, and the message says why.
     */
    add(item: CatalogItem): boolean {
      const result = addFavorite(state.selection, item)
      if (!result.ok) {
        emit({
          message:
            result.reason === 'full'
              ? `You can pick up to ${MAX_PER_CATEGORY} in ${item.category === 'MUSIC' ? 'music' : item.category === 'ANIME' ? 'anime' : 'games'}. Remove one first.`
              : result.reason === 'duplicate'
                ? 'That is already one of your picks.'
                : 'That item could not be added.',
        })
        return false
      }
      emit({ selection: result.selection, message: '' })
      return true
    },
    remove(item: FavoriteItem): void {
      emit({ selection: removeFavorite(state.selection, item.category, item.catalog_item_id), message: '' })
    },
    /** Whether the item is already picked. */
    selected: (item: CatalogItem) => isSelected(state.selection, item),
    /** Whether another pick fits in that category. */
    canAdd: (category: Category) => canAdd(state.selection, category),
    /** The picks in the open tab. */
    picks: () => itemsFor(state.selection, state.category),
    /** Replace the selection from a server read (`core.OwnProfile.favorites`). */
    replaceSelection(value: unknown): boolean {
      const selection = validateSelection(value)
      if (selection === null) {
        emit({ message: 'Your saved picks could not be read.' })
        return false
      }
      emit({ selection, message: '' })
      return true
    },
    /** Clear every category. Skipping all of them is allowed. */
    clear(): void {
      emit({ selection: emptySelection(), message: '' })
    },
    /**
     * Whether the current selection may be saved. An empty selection is
     * savable: skipping every category is allowed and blocks nothing.
     */
    canSave: () => true,
    dispose() {
      if (disposed) return
      disposed = true
      started = false
      epoch++
      flights.forEach((flight) => flight.abort())
      flights.clear()
      listeners.clear()
    },
  }
}

export type FavoritesController = ReturnType<typeof createFavoritesController>
