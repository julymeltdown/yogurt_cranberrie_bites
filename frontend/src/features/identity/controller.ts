import type { SessionScope } from '../../shared/api/http'
import type { PrivacyScope } from '../../shared/query/privacy'
import { TransportError } from '../../shared/api/errors'
import {
  createIdentityApi,
  resultMessage,
  type GenderBucket,
  type IdentityApi,
  type IdentityApiOptions,
  type IdentityVerification,
  type ResultMarker,
  type ReturnPath,
  type VerificationStatus,
} from './api/identity'

/** One line of state a screen renders. Nothing here is a claim value except `claims`. */
export interface IdentityState {
  readonly phase: 'idle' | 'loading' | 'ready' | 'error'
  readonly verification: IdentityVerification | null
  readonly message: string
  /** True while a start or disconnect command is in flight. */
  readonly busy: boolean
  /** The provider URL to navigate to, once a start succeeded. */
  readonly authorizationUrl: string | null
  /** The callback marker, when the landing page supplied one. */
  readonly marker: ResultMarker | null
}

export interface IdentityControllerOptions extends IdentityApiOptions {
  /** The current session scope, or `null` when there is no authenticated session. */
  readonly scope: () => SessionScope | null
  /** The privacy coordinator, when the shell has one. */
  readonly privacy?: {
    capture(): PrivacyScope
    isCurrent(scope: PrivacyScope): boolean
  }
  /** Navigate the browser to the provider. Injected so a test never navigates. */
  readonly navigate?: (url: string) => void
  /** Read the callback marker from the current location. */
  readonly marker?: () => ResultMarker | null
}

const initial = (marker: ResultMarker | null): IdentityState => ({
  phase: 'idle',
  verification: null,
  message: '',
  busy: false,
  authorizationUrl: null,
  marker,
})

/**
 * The identity verification controller.
 *
 * One controller per shell. It owns no cache: the profile screen is a single
 * owner-only view, and a stale claim is worse than a re-read, so every state
 * transition re-reads from the server rather than guessing.
 *
 * The controller never submits anything on the user's behalf after a
 * verification. `marker` is a *report* of what the server said, not a trigger:
 * the task document forbids auto-submitting the work the user was doing before
 * verification, and a landing page that resumed a draft would be exactly that.
 */
export function createIdentityController(options: IdentityControllerOptions) {
  const api: IdentityApi = createIdentityApi(options)
  const now = options.now ?? Date.now
  const listeners = new Set<() => void>()
  let state: IdentityState = initial(options.marker?.() ?? null)
  let disposed = false
  let serial = 0

  const emit = (patch: Partial<IdentityState>) => {
    state = Object.freeze({ ...state, ...patch })
    for (const listener of [...listeners]) {
      try {
        listener()
      } catch {
        // One observer must not stop the others.
      }
    }
  }

  const sessionCurrent = (): boolean => {
    if (disposed) return false
    const scope = options.scope()
    if (!scope || scope.userId === null) return false
    return options.privacy ? options.privacy.isCurrent(options.privacy.capture()) : true
  }

  async function load(): Promise<void> {
    if (!sessionCurrent()) {
      emit({ phase: 'error', verification: null, message: 'Sign in to see your verification.' })
      return
    }
    const attempt = ++serial
    emit({ phase: 'loading', message: '' })
    try {
      const verification = await api.read()
      if (attempt !== serial || !sessionCurrent()) return
      emit({ phase: 'ready', verification, message: '' })
    } catch (error) {
      if (attempt !== serial || !sessionCurrent()) return
      emit({
        phase: 'error',
        verification: null,
        message: failureMessage(error),
      })
    }
  }

  async function start(returnPath: ReturnPath = '/me/identity'): Promise<boolean> {
    if (!sessionCurrent() || state.busy || state.phase === 'loading') return false
    const attempt = ++serial
    emit({ busy: true, message: 'Opening NAVER…' })
    try {
      const started = await api.start(returnPath)
      if (attempt !== serial || !sessionCurrent()) return false
      emit({ busy: false, authorizationUrl: started.authorization_url, message: '' })
      // The navigation is the last step and is injected, so a test asserts the
      // exact URL without leaving the document.
      options.navigate?.(started.authorization_url)
      return true
    } catch (error) {
      if (attempt !== serial || !sessionCurrent()) return false
      emit({ busy: false, message: failureMessage(error) })
      return false
    }
  }

  async function disconnect(): Promise<boolean> {
    if (!sessionCurrent() || state.busy) return false
    const attempt = ++serial
    emit({ busy: true, message: 'Disconnecting…' })
    try {
      const verification = await api.disconnect()
      if (attempt !== serial || !sessionCurrent()) return false
      emit({
        busy: false,
        phase: 'ready',
        verification,
        message: 'NAVER is disconnected. Your posts keep their audience; your age and gender are no longer verified.',
      })
      return true
    } catch (error) {
      if (attempt !== serial || !sessionCurrent()) return false
      emit({ busy: false, message: failureMessage(error) })
      return false
    }
  }

  /** Dismiss the callback report. Nothing else happens: no work is resumed. */
  function dismissMarker(): void {
    emit({ marker: null })
  }

  return {
    read: (): IdentityState => state,
    subscribe(listener: () => void): () => void {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    start: load,
    reload: load,
    startVerification: start,
    disconnect,
    dismissMarker,
    /** The sentence for the current marker, if any. */
    markerMessage(): string {
      return state.marker === null ? '' : resultMessage(state.marker)
    },
    /** Whether the user can begin a verification now. */
    canVerify(): boolean {
      return (
        sessionCurrent() &&
        !state.busy &&
        state.phase !== 'loading' &&
        state.verification !== null
      )
    },
    /** Whether the user can disconnect now. */
    canDisconnect(): boolean {
      return sessionCurrent() && !state.busy && state.verification?.status === 'VERIFIED'
    },
    dispose(): void {
      if (disposed) return
      disposed = true
      serial++
      listeners.clear()
    },
    /** Exposed for the tests that assert the exact request shape. */
    api,
    now,
  }
}
export type IdentityController = ReturnType<typeof createIdentityController>

/** The one-line status label for a verification state. */
export function statusLabel(status: VerificationStatus): string {
  switch (status) {
    case 'VERIFIED':
      return 'Verified with NAVER'
    case 'EXPIRED':
      return 'Verification expired'
    case 'REVOKED':
      return 'NAVER disconnected'
    case 'PROVIDER_UNAVAILABLE':
      return 'Verification status unavailable'
    default:
      return 'Not verified'
  }
}

/** The gender bucket as a sentence, never as a raw value. */
export function genderLabel(bucket: GenderBucket): string {
  switch (bucket) {
    case 'female':
      return 'Female'
    case 'male':
      return 'Male'
    default:
      // `unknown` is what NAVER itself reported as "not specified". It is not a
      // wildcard and it is not "any".
      return 'Not specified by NAVER'
  }
}

/** A provider-independent failure sentence. A raw provider code is never shown. */
export function failureMessage(error: unknown): string {
  if (!(error instanceof TransportError)) return 'Verification is unavailable. Try again later.'
  if (error.status === 429) return 'Too many attempts. Wait a moment before trying again.'
  if (error.status === 401 || error.status === 403) return 'Sign in again to manage your verification.'
  if (error.status === 503 || error.status === 504) return 'Verification is unavailable. Try again later.'
  return 'Verification is unavailable. Try again later.'
}

/** The expiry date as a short, locale-independent date. */
export function expiryLabel(expiresAt: string | null): string {
  if (!expiresAt) return ''
  const date = new Date(expiresAt)
  if (Number.isNaN(date.getTime())) return ''
  return date.toISOString().slice(0, 10)
}
