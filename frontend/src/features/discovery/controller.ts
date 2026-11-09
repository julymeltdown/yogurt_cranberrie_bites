import type { HttpClient } from '../../shared/api/http'
import type { SessionController } from '../auth/model/session'
import {
  blank,
  createControllerRuntime,
  failureOf,
  type Failure,
  type Slot,
} from '../../shared/query/controller-runtime'
import { newIdempotencyKey } from '../../shared/api/request'
import type { Category } from '../favorites/contract'
import {
  DEFAULT_PEOPLE_CATEGORY,
  DISCOVERY_TABS,
  decodePeoplePage,
  type DiscoveryTab,
  type PeopleCard,
  type PeoplePage,
} from './contract'

/**
 * WP-03 T-805: the discovery controller.
 *
 * It serves the **people** list only. The mutuals-intro list is a separate
 * operation with its own audience filter and its own controller; there is no
 * method here that can add an intro post to the people list or a person to the
 * intro list. That separation is the product rule, so it is a type-level fact
 * rather than a rendering convention.
 *
 * The controller also owns the two actions a discovery card offers: opening the
 * profile and following. It deliberately has no "open the room" action — the
 * room is a separate place inside the profile, never a step in discovery.
 */

export interface DiscoveryOptions {
  readonly auth: Pick<SessionController, 'privacy'>
  readonly request: HttpClient['request']
}

export type { Failure, Slot }

export interface DiscoveryState {
  readonly tab: DiscoveryTab
  readonly category: Category
  readonly people: Slot<readonly PeopleCard[]>
  readonly nextCursor: string | null
  readonly following: readonly string[]
  readonly message: string
  readonly available: boolean
}

const failureMessage: Readonly<Record<Failure, string>> = Object.freeze({
  unavailable: 'People are unavailable right now. Try again in a moment.',
  invalid: 'The people list could not be verified.',
  offline: 'Connection unavailable. Retry explicitly.',
  'rate-limited': 'Too many requests. Wait before trying again.',
})

export function createDiscoveryController(options: DiscoveryOptions) {
  const runtime = createControllerRuntime<DiscoveryState>(
    { privacy: options.auth.privacy, request: options.request },
    () => ({
      tab: 'PEOPLE',
      category: DEFAULT_PEOPLE_CATEGORY,
      people: blank<readonly PeopleCard[]>(),
      nextCursor: null,
      following: [],
      message: '',
      available: false,
    }),
  )
  // The runtime publishes through this closure; it is defined before `runtime`
  // runs because the runtime never publishes during construction.
  function emit(patch: Partial<DiscoveryState>) {
    runtime.publish(patch)
  }
  const { run, usable, cancel } = runtime
  const state = () => runtime.read()

  async function load(category: Category, cursor: string | null): Promise<void> {
    emit({
      people: { phase: 'loading', data: cursor === null ? null : state().people.data, failure: null },
      message: '',
    })
    try {
      const page = await run('people', () => ({
        path: '/api/discover/people',
        method: 'GET',
        access: 'protected',
        errorFamily: 'core',
        query: { category, limit: 20, ...(cursor === null ? {} : { cursor }) },
        decode: (value: unknown) => decodePeoplePage(value),
      }))
      if (state().category !== category) return
      const previous = cursor === null ? [] : state().people.data ?? []
      emit({
        people: { phase: 'ready', data: [...previous, ...page.items], failure: null },
        nextCursor: page.next_cursor,
      })
    } catch (error) {
      if (state().category !== category) return
      const reason = failureOf(error)
      emit({
        people: { phase: 'error', data: null, failure: reason },
        nextCursor: null,
        message: failureMessage[reason],
      })
    }
  }

  return {
    read: runtime.read,
    subscribe: runtime.subscribe,
    start: runtime.start,
    /** The top-level tabs: people, then mutuals-intro posts. Never merged. */
    tabs: () => DISCOVERY_TABS,
    selectTab(tab: DiscoveryTab) {
      if (!DISCOVERY_TABS.includes(tab) || state().tab === tab) return
      if (tab === 'MUTUALS_INTROS') cancel('people')
      emit({ tab, message: '' })
    },
    /** Switch the people category. The default tab is music. */
    selectCategory(category: Category) {
      if (state().category === category) return
      cancel('people')
      emit({
        category,
        people: blank<readonly PeopleCard[]>(),
        nextCursor: null,
        message: '',
      })
      if (usable() && state().tab === 'PEOPLE') void load(category, null)
    },
    load(): Promise<void> {
      if (!usable()) return Promise.resolve()
      return load(state().category, null)
    },
    more(): Promise<void> {
      if (!usable() || state().nextCursor === null) return Promise.resolve()
      return load(state().category, state().nextCursor)
    },
    /**
     * Follow a profile. This is an explicit action with its own effect; merely
     * viewing a discovery card never follows anybody.
     */
    async follow(card: PeopleCard): Promise<boolean> {
      if (!usable() || state().following.includes(card.profile_id)) return false
      try {
        await run(`follow:${card.profile_id}`, () => ({
          path: `/api/profiles/${card.profile_id}/follow`,
          method: 'PUT',
          access: 'protected',
          errorFamily: 'domain',
          domainOperation: 'followProfile',
          idempotencyKey: newIdempotencyKey(),
          decode: (value: unknown) => value as { following: boolean },
        }))
        emit({ following: [...state().following, card.profile_id], message: '' })
        return true
      } catch (error) {
        emit({ message: failureMessage[failureOf(error)] })
        return false
      }
    },
    isFollowing: (card: PeopleCard) => state().following.includes(card.profile_id),
    dispose: runtime.dispose,
  }
}

export type DiscoveryController = ReturnType<typeof createDiscoveryController>

/** Re-exported so a caller can type a decoded page without a second import. */
export type { PeoplePage }
