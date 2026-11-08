// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { IntroLikeButton, UNAVAILABLE_PROMPT } from '../IntroLikeButton'
import { likeCountLabel } from '../contract'
import type { LikeTransport } from '../api/intro-likes'

afterEach(cleanup)

const introId = '00000000-0000-4000-8000-0000000000aa'

/** A transport whose responses the test controls. */
function transport(respond: (path: string, method: string) => Promise<unknown>) {
  const calls: { path: string; method: string }[] = []
  const value: LikeTransport = {
    request: (async (request: { path: string; method: string }) => {
      calls.push({ path: request.path, method: request.method })
      const data = await respond(request.path, request.method)
      return { data, status: 200, headers: new Headers() }
    }) as unknown as LikeTransport['request'],
  }
  return { transport: value, calls }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((yes) => {
    resolve = yes
  })
  return { promise, resolve }
}

describe('IntroLikeButton', () => {
  it('reflects the current state in aria-pressed', () => {
    const { transport: t } = transport(async () => ({ liked: true, like_count: 1 }))
    const { rerender } = render(
      <IntroLikeButton introId={introId} initial={{ count: 0, liked: false }} transport={t} />,
    )
    expect(screen.getByRole('button').getAttribute('aria-pressed')).toBe('false')
    rerender(<IntroLikeButton introId={introId} initial={{ count: 3, liked: true }} transport={t} />)
    expect(screen.getByRole('button').getAttribute('aria-pressed')).toBe('true')
  })

  // The acceptance case: double-tapping sends one request.
  it('sends one request for a double tap', async () => {
    const pending = deferred<{ liked: boolean; like_count: number }>()
    const { transport: t, calls } = transport(() => pending.promise)
    render(<IntroLikeButton introId={introId} initial={{ count: 0, liked: false }} transport={t} />)
    const button = screen.getByRole('button')
    fireEvent.click(button)
    fireEvent.click(button)
    fireEvent.click(button)
    expect(calls).toHaveLength(1)
    pending.resolve({ liked: true, like_count: 1 })
    await waitFor(() => expect(screen.getByRole('button').getAttribute('aria-pressed')).toBe('true'))
    expect(calls).toHaveLength(1)
  })

  // The acceptance case: a failed like reverts the visual state.
  it('reverts the visual state when the request fails', async () => {
    const { transport: t } = transport(async () => {
      throw new Error('boom')
    })
    render(<IntroLikeButton introId={introId} initial={{ count: 2, liked: false }} transport={t} />)
    fireEvent.click(screen.getByRole('button'))
    // Optimistic: the heart flips immediately.
    expect(screen.getByRole('button').getAttribute('aria-pressed')).toBe('true')
    await waitFor(() => expect(screen.getByRole('button').getAttribute('aria-pressed')).toBe('false'))
    // The count is back to what it was.
    expect(document.querySelector('.intro-like-count')!.textContent).toBe('2')
    expect(screen.getByRole('alert').textContent).toContain('did not go through')
  })

  it('clears a heart with DELETE', async () => {
    const { transport: t, calls } = transport(async () => ({ liked: false, like_count: 1 }))
    render(<IntroLikeButton introId={introId} initial={{ count: 2, liked: true }} transport={t} />)
    fireEvent.click(screen.getByRole('button'))
    await waitFor(() => expect(screen.getByRole('button').getAttribute('aria-pressed')).toBe('false'))
    expect(calls).toEqual([{ path: `/api/intro-posts/${introId}/like`, method: 'DELETE' }])
    expect(document.querySelector('.intro-like-count')!.textContent).toBe('1')
  })

  // The acceptance case: a 404 shows the neutral prompt, not an error toast.
  it('shows the neutral prompt on a 404', async () => {
    const onUnavailable = vi.fn()
    const { transport: t } = transport(async () => {
      const { TransportError } = await import('../../../shared/api/errors')
      throw new TransportError('http', { status: 404 })
    })
    render(
      <IntroLikeButton
        introId={introId}
        initial={{ count: 4, liked: false }}
        transport={t}
        onUnavailable={onUnavailable}
      />,
    )
    fireEvent.click(screen.getByRole('button'))
    await waitFor(() => expect(screen.getByText(UNAVAILABLE_PROMPT)).toBeTruthy())
    // It is a status, not an alert: this is not an error the user caused.
    expect(screen.getByRole('status').textContent).toBe(UNAVAILABLE_PROMPT)
    expect(screen.queryByRole('alert')).toBeNull()
    expect(onUnavailable).toHaveBeenCalledTimes(1)
    // The heart is back where it was, and the button stops offering it.
    expect(screen.getByRole('button').getAttribute('aria-pressed')).toBe('false')
    expect((screen.getByRole('button') as HTMLButtonElement).disabled).toBe(true)
    // The count is unchanged.
    expect(document.querySelector('.intro-like-count')!.textContent).toBe('4')
  })

  // The acceptance case: null count is never rendered as zero.
  it('renders a null count as an em dash, never zero', () => {
    expect(likeCountLabel(null)).toBe('\u2014')
    expect(likeCountLabel(0)).toBe('0')
    expect(likeCountLabel(7)).toBe('7')
    const { transport: t } = transport(async () => ({ liked: false, like_count: null }))
    render(<IntroLikeButton introId={introId} initial={{ count: null, liked: true }} transport={t} />)
    expect(document.querySelector('.intro-like-count')!.textContent).toBe('\u2014')
    expect(document.querySelector('.intro-like-count')!.textContent).not.toBe('0')
  })

  // The `DELETE`-after-access-lost path: the server returns a null count and the
  // button renders the dash rather than claiming zero.
  it('renders the dash after a delete that returns a null count', async () => {
    const { transport: t } = transport(async () => ({ liked: false, like_count: null }))
    render(<IntroLikeButton introId={introId} initial={{ count: 5, liked: true }} transport={t} />)
    fireEvent.click(screen.getByRole('button'))
    await waitFor(() => expect(document.querySelector('.intro-like-count')!.textContent).toBe('\u2014'))
  })

  it('moves the count optimistically and accepts the server value', async () => {
    const pending = deferred<{ liked: boolean; like_count: number }>()
    const { transport: t } = transport(() => pending.promise)
    render(<IntroLikeButton introId={introId} initial={{ count: 4, liked: false }} transport={t} />)
    fireEvent.click(screen.getByRole('button'))
    expect(document.querySelector('.intro-like-count')!.textContent).toBe('5')
    // The server disagrees: it says 9. The server wins.
    pending.resolve({ liked: true, like_count: 9 })
    await waitFor(() => expect(document.querySelector('.intro-like-count')!.textContent).toBe('9'))
  })

  it('does not move a null count optimistically', () => {
    const { transport: t } = transport(async () => ({ liked: true, like_count: 1 }))
    render(<IntroLikeButton introId={introId} initial={{ count: null, liked: false }} transport={t} />)
    fireEvent.click(screen.getByRole('button'))
    // Still the dash: there was no number to move.
    expect(document.querySelector('.intro-like-count')!.textContent).toBe('\u2014')
  })

  it('has an accessible name that reflects the action', () => {
    const { transport: t } = transport(async () => ({ liked: true, like_count: 1 }))
    const { rerender } = render(
      <IntroLikeButton introId={introId} initial={{ count: 0, liked: false }} transport={t} />,
    )
    expect(screen.getByRole('button', { name: 'Leave a heart' })).toBeTruthy()
    rerender(<IntroLikeButton introId={introId} initial={{ count: 1, liked: true }} transport={t} />)
    expect(screen.getByRole('button', { name: 'Remove your heart' })).toBeTruthy()
  })
})
