import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { TransportError } from '../../shared/api/errors'
import { clearLike, setLike, type LikeTransport } from './api/intro-likes'
import { likeCountLabel, type IntroLikeState } from './contract'

/**
 * WP-05 T-411: the heart button.
 *
 * Five behaviours, each with a test:
 *
 * 1. **`aria-pressed` reflects the current state.** It is the accessible name of
 *    the toggle, so a screen reader announces "pressed" or "not pressed" rather
 *    than reading a glyph.
 * 2. **An in-flight guard prevents duplicate requests.** Double-tapping sends one
 *    request; the second tap is ignored until the first settles.
 * 3. **The update is optimistic with rollback.** The heart flips immediately and
 *    reverts if the request fails, so the user is never shown a state the server
 *    did not accept.
 * 4. **A `404` shows the neutral prompt, not an error toast.** A post that
 *    disappeared or became invisible is not a failure the user caused; the
 *    button explains that the post is unavailable and stops offering the heart.
 * 5. **`like_count: null` renders as an em dash, never `0`.** `null` means the
 *    viewer may no longer see the count; `0` would be a claim.
 */
export interface IntroLikeButtonProps {
  readonly introId: string
  readonly initial: { readonly count: number | null; readonly liked: boolean }
  readonly transport: LikeTransport
  /** Called when the server refuses the like because the post is unavailable. */
  readonly onUnavailable?: () => void
}

export interface LikeViewState {
  readonly liked: boolean
  readonly count: number | null
  readonly pending: boolean
  readonly unavailable: boolean
  readonly message: string
}

/** The neutral prompt a `404` produces. */
export const UNAVAILABLE_PROMPT = 'This post is no longer available to you.'

export function IntroLikeButton({ introId, initial, transport, onUnavailable }: IntroLikeButtonProps) {
  const [state, setState] = useState<LikeViewState>(() => ({
    liked: initial.liked,
    count: initial.count,
    pending: false,
    unavailable: false,
    message: '',
  }))
  // The in-flight guard. A ref, not state, because the guard must be synchronous:
  // two taps in the same tick both read the same state snapshot otherwise.
  const inFlight = useRef(false)
  // The last `initial` this component rendered. A card whose post is refreshed
  // (a list reload, a re-read after a navigation) passes a new `initial`, and the
  // button must adopt it rather than keep a stale local copy. It is compared by
  // value so an unchanged prop does not clobber an optimistic update.
  const seen = useRef({ count: initial.count, liked: initial.liked })
  if (seen.current.count !== initial.count || seen.current.liked !== initial.liked) {
    seen.current = { count: initial.count, liked: initial.liked }
    // Adopt the new server value unless a request is in flight, in which case the
    // request's own result will win when it settles.
    if (!inFlight.current) {
      setState((current) =>
        current.pending
          ? current
          : { ...current, liked: initial.liked, count: initial.count, unavailable: false, message: '' },
      )
    }
  }
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  const toggle = useCallback(async () => {
    if (inFlight.current || state.unavailable) return
    inFlight.current = true
    // Optimistic: flip the heart and move the count by one, if there is a count.
    const previous = { liked: state.liked, count: state.count }
    const nextLiked = !previous.liked
    setState({
      liked: nextLiked,
      count: previous.count === null ? null : Math.max(0, previous.count + (nextLiked ? 1 : -1)),
      pending: true,
      unavailable: false,
      message: '',
    })
    try {
      const result: IntroLikeState = nextLiked
        ? await setLike(transport, introId)
        : await clearLike(transport, introId)
      if (!mounted.current) return
      setState({
        liked: result.liked,
        count: result.like_count,
        pending: false,
        unavailable: false,
        message: '',
      })
    } catch (error) {
      if (!mounted.current) return
      if (error instanceof TransportError && error.status === 404) {
        // The post is not available. That is not an error the user caused, so the
        // button shows a neutral prompt rather than an error message.
        setState({
          liked: previous.liked,
          count: previous.count,
          pending: false,
          unavailable: true,
          message: UNAVAILABLE_PROMPT,
        })
        onUnavailable?.()
        return
      }
      // Roll back to exactly what the user saw before the tap.
      setState({
        liked: previous.liked,
        count: previous.count,
        pending: false,
        unavailable: false,
        message: 'That did not go through. Try again.',
      })
    } finally {
      inFlight.current = false
    }
  }, [introId, onUnavailable, state.count, state.liked, state.unavailable, transport])

  return (
    <div className="intro-like">
      <button
        type="button"
        className={state.liked ? 'intro-like-button intro-like-button-active' : 'intro-like-button'}
        aria-pressed={state.liked}
        aria-label={state.liked ? 'Remove your heart' : 'Leave a heart'}
        disabled={state.pending || state.unavailable}
        onClick={() => void toggle()}
      >
        <span aria-hidden="true">{state.liked ? '\u2665' : '\u2661'}</span>
      </button>
      <span className="intro-like-count" aria-live="polite">
        {likeCountLabel(state.count)}
      </span>
      {state.message !== '' && (
        <p
          className={state.unavailable ? 'intro-like-note' : 'intro-like-error'}
          role={state.unavailable ? 'status' : 'alert'}
        >
          {state.message}
        </p>
      )}
    </div>
  )
}

/**
 * A store-backed variant for a card that is re-rendered by an external
 * controller. It exists so the card and the detail can share one implementation.
 */
export interface LikeStore {
  read(): { readonly count: number | null; readonly liked: boolean }
  subscribe(listener: () => void): () => void
}

export function IntroLikeButtonFromStore({
  introId,
  store,
  transport,
  onUnavailable,
}: {
  readonly introId: string
  readonly store: LikeStore
  readonly transport: LikeTransport
  readonly onUnavailable?: () => void
}) {
  const value = useSyncExternalStore(store.subscribe, store.read, store.read)
  return (
    <IntroLikeButton
      introId={introId}
      initial={{ count: value.count, liked: value.liked }}
      transport={transport}
      {...(onUnavailable === undefined ? {} : { onUnavailable })}
    />
  )
}
