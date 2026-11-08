import type { HttpClient } from '../../shared/api/http'
import { TransportError } from '../../shared/api/errors'
import type { SessionController } from '../auth/model/session'
import type { OwnIntroPost } from './contract'
import { publishIntroPost, saveIntroPostDraft } from './api/intro-posts'

/**
 * WP-05 T-408: the composer controller.
 *
 * It owns exactly the two remote actions the composer has, and nothing else:
 *
 * * `saveDraft` — persist the draft and return the server's projection;
 * * `publish` — publish a stored draft.
 *
 * Both return the server's `OwnIntroPost`, which is where `audience_summary`
 * comes from. The composer never computes a summary; it renders the string this
 * controller hands back.
 *
 * The controller also reports `busy` so the composer can disable its actions
 * while a request is in flight, and it generates one idempotency key per
 * attempt, so a retried save is not applied twice.
 */
export interface IntroComposerOptions {
  readonly auth: Pick<SessionController, 'privacy'>
  readonly request: HttpClient['request']
  /** Deterministic fixtures only; production uses `crypto.randomUUID`. */
  readonly key?: () => string
}

export interface ComposerRemoteState {
  readonly busy: boolean
  readonly publishing: boolean
  readonly message: string
  readonly available: boolean
}

/** The subset of `OwnIntroPost` the composer needs back. */
export interface SavedIntro {
  readonly introId: string
  readonly revision: number
  readonly audienceSummary: string
}

export interface IntroComposerController {
  read(): ComposerRemoteState
  subscribe(listener: () => void): () => void
  start(): void
  saveDraft(
    body: unknown,
    accept: (saved: SavedIntro) => void,
  ): Promise<boolean>
  publish(
    introId: string,
    expectedRevision: number,
    accept: (saved: SavedIntro) => void,
  ): Promise<boolean>
  dispose(): void
}

function failureMessage(error: unknown): string {
  if (!(error instanceof TransportError)) return 'That could not be saved. Check your input and try again.'
  if (error.status === 409) return 'This post changed elsewhere. Load the latest version before saving.'
  if (error.status === 400) return 'Check your introduction and audience, then try again.'
  if (error.status === 429) return 'Too many requests. Wait a moment before trying again.'
  if (error.kind === 'network' || error.kind === 'timeout') {
    return 'Connection unavailable. Your draft is preserved.'
  }
  return 'That could not be saved. Try again in a moment.'
}

export function createIntroComposerController(options: IntroComposerOptions): IntroComposerController {
  let state: ComposerRemoteState = Object.freeze({
    busy: false,
    publishing: false,
    message: '',
    available: false,
  })
  let started = false
  let disposed = false
  const listeners = new Set<() => void>()
  const key = options.key ?? (() => crypto.randomUUID())
  const emit = (patch: Partial<ComposerRemoteState>) => {
    state = Object.freeze({ ...state, ...patch })
    for (const listener of [...listeners]) listener()
  }
  const usable = () => {
    if (!started || disposed) return false
    return options.auth.privacy.isCurrent(options.auth.privacy.capture())
  }
  const saved = (post: OwnIntroPost): SavedIntro =>
    Object.freeze({
      introId: post.intro_id,
      revision: post.revision,
      // The server's string, verbatim.
      audienceSummary: post.audience_summary,
    })

  return {
    read: () => state,
    subscribe(listener) {
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
    async saveDraft(body, accept) {
      if (!usable() || state.busy) return false
      const payload = body as { introId?: string } & Record<string, unknown>
      const introId = typeof payload.introId === 'string' ? payload.introId : crypto.randomUUID()
      emit({ busy: true, message: '' })
      try {
        const post = await saveIntroPostDraft(
          { request: options.request },
          introId,
          payload as never,
          key(),
        )
        const result = saved(post)
        accept(result)
        emit({ busy: false })
        return true
      } catch (error) {
        emit({ busy: false, message: failureMessage(error) })
        return false
      }
    },
    async publish(introId, expectedRevision, accept) {
      if (!usable() || state.busy) return false
      emit({ busy: true, publishing: true, message: '' })
      try {
        const post = await publishIntroPost(
          { request: options.request },
          introId,
          expectedRevision,
          key(),
        )
        const result = saved(post)
        accept(result)
        emit({ busy: false, publishing: false })
        return true
      } catch (error) {
        emit({ busy: false, publishing: false, message: failureMessage(error) })
        return false
      }
    },
    dispose() {
      disposed = true
      started = false
      listeners.clear()
    },
  }
}
