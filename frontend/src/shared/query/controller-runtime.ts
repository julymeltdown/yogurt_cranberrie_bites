/**
 * The one "load → error → retry" runtime the feature controllers share.
 *
 * `favorites` and `discovery` were written independently and ended up with the
 * same lifecycle, line for line:
 *
 * * a `Slot<T>` (`idle | loading | ready | error` plus a `Failure` reason);
 * * a `failure()` classifier that maps a `TransportError` onto that reason;
 * * a listener set with isolated observers;
 * * a `start`/`dispose` pair that binds to the privacy scope;
 * * one `run(lane, build)` that cancels the previous flight on the same lane,
 *   stamps the request with an abort signal, and refuses a result that arrives
 *   after the scope or the epoch moved.
 *
 * The duplication was not cosmetic: the two copies had already drifted, so a
 * fix to one (say, cancelling on a scope change) never reached the other. This
 * module is that lifecycle, once. It is deliberately *not* a controller base
 * class: each controller still owns its state shape, its actions and its
 * messages, and calls into this runtime for the parts that must not drift.
 */
import type { HttpRequest } from '../api/http'
import { TransportError } from '../api/errors'
import type { PrivacyScope } from '../query/privacy'

/** The closed failure vocabulary a slot can report. */
export type Failure = 'unavailable' | 'invalid' | 'offline' | 'rate-limited'

/** One async read's state. `data` survives a reload so a list can append. */
export interface Slot<T> {
  readonly phase: 'idle' | 'loading' | 'ready' | 'error'
  readonly data: T | null
  readonly failure: Failure | null
}

/** An empty slot. */
export const blank = <T>(): Slot<T> => ({ phase: 'idle', data: null, failure: null })

/**
 * Classify one failure.
 *
 * The order matters and is the product's: a `429` is a rate limit before it is a
 * generic refusal, a network/timeout failure is offline, a `400` is the caller's
 * input, and everything else — including the product's opaque `404` — is
 * "unavailable".
 */
export function failureOf(error: unknown): Failure {
  if (!(error instanceof TransportError)) return 'invalid'
  if (error.status === 429) return 'rate-limited'
  if (error.kind === 'network' || error.kind === 'timeout') return 'offline'
  if (error.status === 400) return 'invalid'
  return 'unavailable'
}

/** The privacy port every controller needs. */
export interface PrivacyPort {
  capture(): PrivacyScope
  isCurrent(scope: PrivacyScope): boolean
}

export interface ControllerRuntimeOptions {
  readonly privacy: PrivacyPort
  /** The transport entry point. The runtime stamps each request with its signal. */
  readonly request: <T>(request: HttpRequest<T>) => Promise<{ readonly data: T; readonly scope: unknown }>
}

/**
 * Create one controller's runtime.
 *
 * `emit` is the controller's own state publisher: the runtime never owns the
 * state shape, it only decides *when* the controller may publish.
 */
export function createControllerRuntime<TState extends { readonly available: boolean }>(
  options: ControllerRuntimeOptions,
  initial: () => TState,
) {
  let state = initial()
  let started = false
  let disposed = false
  let epoch = 0
  const flights = new Map<string, AbortController>()
  const listeners = new Set<() => void>()

  const publish = (patch: Partial<TState>) => {
    state = Object.freeze({ ...state, ...patch }) as TState
    // The snapshot is deliberate: a listener that subscribes or unsubscribes
    // during this publish must not change the set mid-iteration.
    const snapshot = [...listeners]
    for (const listener of snapshot) {
      try {
        listener()
      } catch {
        // Isolate observers so one bad subscriber cannot stop the others.
      }
    }
  }

  const scope = (): PrivacyScope | null => {
    const captured = options.privacy.capture()
    return options.privacy.isCurrent(captured) ? captured : null
  }

  const usable = () => started && !disposed && scope() !== null

  /** Abort the flight on `lane`, or every flight when no lane is named. */
  function cancel(lane?: string) {
    if (lane !== undefined) {
      flights.get(lane)?.abort()
      flights.delete(lane)
      return
    }
    epoch += 1
    for (const flight of flights.values()) flight.abort()
    flights.clear()
  }

  /**
   * Run one request on a lane.
   *
   * The previous flight on the same lane is cancelled first, so the newest
   * request wins. A result that arrives after the scope changed, after the epoch
   * moved, or after a newer request took the lane is discarded as
   * `session-changed` rather than published into the wrong account.
   */
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
      if (version !== epoch || !options.privacy.isCurrent(captured)) {
        throw new TransportError('session-changed')
      }
      return result.data
    } finally {
      if (flights.get(lane) === flight) flights.delete(lane)
    }
  }

  return {
    read: () => state,
    publish,
    subscribe(listener: () => void) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    /** Mark the controller usable and report it. Idempotent. */
    start() {
      if (started || disposed) return
      started = true
      publish({ available: usable() } as Partial<TState>)
    },
    /** Whether a request may be made right now. */
    usable,
    scope,
    cancel,
    run,
    dispose() {
      if (disposed) return
      disposed = true
      started = false
      cancel()
      listeners.clear()
    },
  }
}

export type ControllerRuntime<TState extends { readonly available: boolean }> =
  ReturnType<typeof createControllerRuntime<TState>>
