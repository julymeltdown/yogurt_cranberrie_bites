import type { HttpClient, HttpRequest } from '../../shared/api/http'
import { TransportError } from '../../shared/api/errors'
import type { SessionController } from '../auth/model/session'
import type { PrivacyScope } from '../../shared/query/privacy'
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

export type Failure = 'unavailable' | 'invalid' | 'offline' | 'rate-limited'

export interface Slot<T> {
  readonly phase: 'idle' | 'loading' | 'ready' | 'error'
  readonly data: T | null
  readonly failure: Failure | null
}

export interface DiscoveryState {
  readonly tab: DiscoveryTab
  readonly category: Category
  readonly people: Slot<readonly PeopleCard[]>
  readonly nextCursor: string | null
  readonly following: readonly string[]
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
  unavailable: 'People are unavailable right now. Try again in a moment.',
  invalid: 'The people list could not be verified.',
  offline: 'Connection unavailable. Retry explicitly.',
  'rate-limited': 'Too many requests. Wait before trying again.',
})

export function createDiscoveryController(options: DiscoveryOptions) {
  let state: DiscoveryState = Object.freeze({
    tab: 'PEOPLE',
    category: DEFAULT_PEOPLE_CATEGORY,
    people: blank<readonly PeopleCard[]>(),
    nextCursor: null,
    following: [],
    message: '',
    available: false,
  })
  let disposed = false
  let started = false
  let epoch = 0
  const flights = new Map<string, AbortController>()
  const listeners = new Set<() => void>()

  const emit = (patch: Partial<DiscoveryState>) => {
    state = Object.freeze({ ...state, ...patch })
    for (const listener of [...listeners]) {
      try {
        listener()
      } catch {
        // Isolate observers.
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
      const result = await options.request({ ...build(), signal: flight.signal })
      if (version !== epoch || !options.auth.privacy.isCurrent(captured)) {
        throw new TransportError('session-changed')
      }
      return result.data
    } finally {
      if (flights.get(lane) === flight) flights.delete(lane)
    }
  }

  async function load(category: Category, cursor: string | null): Promise<void> {
    emit({
      people: { phase: 'loading', data: cursor === null ? null : state.people.data, failure: null },
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
      if (state.category !== category) return
      const previous = cursor === null ? [] : state.people.data ?? []
      emit({
        people: { phase: 'ready', data: [...previous, ...page.items], failure: null },
        nextCursor: page.next_cursor,
      })
    } catch (error) {
      if (state.category !== category) return
      const reason = failure(error)
      emit({
        people: { phase: 'error', data: null, failure: reason },
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
    /** The top-level tabs: people, then mutuals-intro posts. Never merged. */
    tabs: () => DISCOVERY_TABS,
    selectTab(tab: DiscoveryTab) {
      if (!DISCOVERY_TABS.includes(tab) || state.tab === tab) return
      if (tab === 'MUTUALS_INTROS') cancel('people')
      emit({ tab, message: '' })
    },
    /** Switch the people category. The default tab is music. */
    selectCategory(category: Category) {
      if (state.category === category) return
      cancel('people')
      emit({
        category,
        people: blank<readonly PeopleCard[]>(),
        nextCursor: null,
        message: '',
      })
      if (usable() && state.tab === 'PEOPLE') void load(category, null)
    },
    load(): Promise<void> {
      if (!usable()) return Promise.resolve()
      return load(state.category, null)
    },
    more(): Promise<void> {
      if (!usable() || state.nextCursor === null) return Promise.resolve()
      return load(state.category, state.nextCursor)
    },
    /**
     * Follow a profile. This is an explicit action with its own effect; merely
     * viewing a discovery card never follows anybody.
     */
    async follow(card: PeopleCard): Promise<boolean> {
      if (!usable() || state.following.includes(card.profile_id)) return false
      try {
        await run(`follow:${card.profile_id}`, () => ({
          path: `/api/profiles/${card.profile_id}/follow`,
          method: 'PUT',
          access: 'protected',
          errorFamily: 'domain',
          domainOperation: 'followProfile',
          idempotencyKey: newId(),
          decode: (value: unknown) => value as { following: boolean },
        }))
        emit({ following: [...state.following, card.profile_id], message: '' })
        return true
      } catch (error) {
        emit({ message: failureMessage[failure(error)] })
        return false
      }
    },
    isFollowing: (card: PeopleCard) => state.following.includes(card.profile_id),
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

function newId(): string {
  const crypto = globalThis.crypto
  if (crypto && typeof crypto.randomUUID === 'function') return crypto.randomUUID()
  // A deterministic fallback keeps the transport contract satisfied in a
  // non-browser test environment; it is never used to derive authority.
  const bytes = Array.from({ length: 16 }, () => Math.floor(Math.random() * 256))
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x40
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80
  const hex = bytes.map((byte) => byte.toString(16).padStart(2, '0')).join('')
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

export type DiscoveryController = ReturnType<typeof createDiscoveryController>

/** Re-exported so a caller can type a decoded page without a second import. */
export type { PeoplePage }
