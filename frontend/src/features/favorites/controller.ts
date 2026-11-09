import type { SessionController } from '../auth/model/session'
import type { HttpClient } from '../../shared/api/http'
import {
  blank,
  createControllerRuntime,
  failureOf,
  type Failure,
  type Slot,
} from '../../shared/query/controller-runtime'
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

export type { Failure, Slot }

export interface FavoritesState {
  readonly category: Category
  readonly term: string
  readonly results: Slot<readonly CatalogItem[]>
  readonly nextCursor: string | null
  readonly selection: FavoriteSelection
  readonly message: string
  readonly available: boolean
}

const failureMessage: Readonly<Record<Failure, string>> = Object.freeze({
  unavailable: 'The catalog is unavailable. Try again in a moment.',
  invalid: 'That search could not be sent. Check the term and try again.',
  offline: 'Connection unavailable. Retry explicitly.',
  'rate-limited': 'Too many searches. Wait before trying again.',
})

export function createFavoritesController(options: FavoritesOptions) {
  const runtime = createControllerRuntime<FavoritesState>(
    { privacy: options.auth.privacy, request: options.request },
    () => ({
      category: 'MUSIC',
      term: '',
      results: blank<readonly CatalogItem[]>(),
      nextCursor: null,
      selection: emptySelection(),
      message: '',
      available: false,
    }),
  )
  // The runtime publishes through this closure; it is defined before `runtime`
  // runs because the runtime never publishes during construction.
  function emit(patch: Partial<FavoritesState>) {
    runtime.publish(patch)
  }
  const { run, usable, cancel } = runtime
  const state = () => runtime.read()

  async function search(category: Category, term: string, cursor: string | null): Promise<void> {
    const query = validQuery(term)
    if (query === null) {
      emit({ results: { phase: 'idle', data: null, failure: null }, nextCursor: null })
      return
    }
    emit({
      results: { phase: 'loading', data: cursor === null ? null : state().results.data, failure: null },
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
      if (state().category !== category) return
      const previous = cursor === null ? [] : state().results.data ?? []
      const merged = [...previous, ...page.items]
      emit({
        results: { phase: 'ready', data: merged, failure: null },
        nextCursor: page.next_cursor,
      })
    } catch (error) {
      if (state().category !== category) return
      const reason = failureOf(error)
      emit({
        results: { phase: 'error', data: null, failure: reason },
        nextCursor: null,
        message: failureMessage[reason],
      })
    }
  }

  return {
    read: runtime.read,
    subscribe: runtime.subscribe,
    start: runtime.start,
    /** Switch tab. The order of the tabs is the product's, not the caller's. */
    selectCategory(category: Category) {
      if (!CATEGORIES.includes(category) || state().category === category) return
      cancel(`catalog:${state().category}`)
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
      return search(state().category, state().term, null)
    },
    more(): Promise<void> {
      if (!usable() || state().nextCursor === null) return Promise.resolve()
      return search(state().category, state().term, state().nextCursor)
    },
    /**
     * Add an item to its own category. A sixth item in one category is refused
     * here, before any request, and the message says why.
     */
    add(item: CatalogItem): boolean {
      const result = addFavorite(state().selection, item)
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
      emit({ selection: removeFavorite(state().selection, item.category, item.catalog_item_id), message: '' })
    },
    /** Whether the item is already picked. */
    selected: (item: CatalogItem) => isSelected(state().selection, item),
    /** Whether another pick fits in that category. */
    canAdd: (category: Category) => canAdd(state().selection, category),
    /** The picks in the open tab. */
    picks: () => itemsFor(state().selection, state().category),
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
    dispose: runtime.dispose,
  }
}

export type FavoritesController = ReturnType<typeof createFavoritesController>
