// @vitest-environment jsdom
// WP-06 T-510: upload failure and cancellation handling.
//
// The acceptance statements are:
//
// * no orphan asset after cancellation;
// * the composer retains the description after a failed upload.
//
// "No orphan" is a claim about what the client *sent*. The client cannot delete
// a `PENDING` row — the frozen contract has no such operation — so the guarantee
// is that a cancelled upload never reaches `finalize` and never creates a post:
// the only row it can leave behind is a `PENDING` intent with no bytes, which the
// server's orphan window erases. The tests below assert exactly that, and the
// server half is `backend/services/concert-service/tests/profile_media_upload.rs`
// (`a_cancelled_upload_leaves_no_orphan_once_the_window_passes`).
import { readFileSync } from 'node:fs'
import { URL as FileURL } from 'node:url'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { createProfileMediaComposer } from '../controller'
import { ProfileMediaComposer } from '../ProfileMediaComposer'
import { classifyFailure, failureMessage, retryable, type UploadState } from '../upload'
import { BYTES, failure, json, media, post, setup, type Fixture } from './fixtures'

const cleanups: (() => void)[] = []
afterEach(() => { cleanups.splice(0).reverse().forEach(fn => fn()); cleanup() })

const MEDIA = '00000000-0000-4000-8000-0000000009aa'

function composer(f: Fixture) {
  const value = createProfileMediaComposer({
    auth: f.auth,
    scope: f.scope,
    digest: f.digest,
    nextLocalId: f.nextLocalId,
    idempotencyKey: (() => {
      let counter = 0
      return () => `00000000-0000-4000-8000-${String(++counter).padStart(12, '0')}`
    })(),
  })
  cleanups.push(value.dispose)
  return value
}

function pick(value: ReturnType<typeof composer>) {
  value.addAttachment({
    localId: 'local-photo.png',
    fileName: 'photo.png',
    contentType: 'image/png',
    size: BYTES.byteLength,
    width: 1200,
    height: 800,
    altText: 'a photograph',
  })
  value.provideSource('local-photo.png', async () => BYTES)
}

/** A server whose upload steps succeed. */
function uploadServer(f: Fixture) {
  f.handler((_request, call) => {
    if (call.path === '/api/me/media/uploads') {
      return json({
        media_id: MEDIA,
        state: 'PENDING',
        upload_path: `/api/me/media/${MEDIA}/bytes`,
        max_bytes: 10 * 1024 * 1024,
        expires_at: '2026-11-05T00:15:00Z',
        upload_method: 'PUT',
        revision: 1,
      })
    }
    if (call.path.endsWith('/bytes')) return json({ media_id: MEDIA, state: 'UPLOADED', revision: 2 })
    if (call.path.endsWith('/finalize')) return json(media())
    if (call.path === '/api/me/posts') return json(post())
    throw new Error(`unexpected ${call.path}`)
  })
}

const paths = (f: Fixture) => f.calls.map(call => `${call.method} ${call.path}`)

describe('a failed upload keeps the composer state and offers retry', () => {
  it('keeps the description, the visibility and the alt text after a failed publish', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    uploadServer(f)
    // The final create fails: the upload itself succeeded.
    f.handler((request, call) => {
      if (call.path === '/api/me/posts') return failure(503, 'DEPENDENCY_UNAVAILABLE')
      return uploadServer.length === 0 ? json({}) : ((): Response => {
        if (call.path === '/api/me/media/uploads') {
          return json({
            media_id: MEDIA,
            state: 'PENDING',
            upload_path: `/api/me/media/${MEDIA}/bytes`,
            max_bytes: 10 * 1024 * 1024,
            expires_at: '2026-11-05T00:15:00Z',
            upload_method: 'PUT',
            revision: 1,
          })
        }
        if (call.path.endsWith('/bytes')) return json({ media_id: MEDIA, state: 'UPLOADED', revision: 2 })
        if (call.path.endsWith('/finalize')) return json(media())
        throw new Error(`unexpected ${request.method} ${call.path}`)
      })()
    })
    const value = composer(f)
    pick(value)
    value.edit({ description: 'a description I typed', visibility: 'PRIVATE' })
    const created = await value.publishPost()
    expect(created).toBeNull()
    expect(value.read().phase).toBe('failed')
    // The draft is exactly what the user typed.
    expect(value.read().draft.description).toBe('a description I typed')
    expect(value.read().draft.visibility).toBe('PRIVATE')
    expect(value.read().attachments[0]?.altText).toBe('a photograph')
    expect(value.read().notice).toContain('Your draft is kept')
    // Retrying sends the same draft again.
    const before = f.calls.length
    await value.publishPost()
    expect(f.calls.length).toBeGreaterThan(before)
    expect((f.calls.at(-1)!.body as { description: string }).description).toBe('a description I typed')
  })

  it('keeps the draft when the upload itself fails', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    f.handler((_request, call) => {
      if (call.path === '/api/me/media/uploads') return failure(503, 'DEPENDENCY_UNAVAILABLE')
      throw new Error(`unexpected ${call.path}`)
    })
    const value = composer(f)
    pick(value)
    value.edit({ description: 'kept through a failed upload' })
    const created = await value.publishPost()
    expect(created).toBeNull()
    expect(value.read().phase).toBe('failed')
    expect(value.read().draft.description).toBe('kept through a failed upload')
    // No post was created.
    expect(paths(f)).toEqual(['POST /api/me/media/uploads'])
    expect(value.read().published).toBeNull()
  })

  it('renders the failure and the retry affordance', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    f.handler(() => failure(503, 'DEPENDENCY_UNAVAILABLE'))
    const value = composer(f)
    pick(value)
    render(<ProfileMediaComposer composer={value} />)
    fireEvent.change(screen.getByLabelText('Description'), { target: { value: 'still here' } })
    fireEvent.click(screen.getByRole('button', { name: 'Publish post' }))
    await waitFor(() => expect(value.read().phase).toBe('failed'))
    // The outcome line is the live region that carries the notice.
    expect(screen.getByText(/Your draft is kept/, { selector: '[role="status"]' })).toBeTruthy()
    expect((screen.getByLabelText('Description') as HTMLTextAreaElement).value).toBe('still here')
    // The publish button is available again: retry is one click.
    expect((screen.getByRole('button', { name: 'Publish post' }) as HTMLButtonElement).disabled).toBe(false)
  })
})

describe('a cancelled upload leaves no orphan', () => {
  it('cancelling before finalize never publishes and never creates a post', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    let releaseBytes: (() => void) | null = null
    f.handler((_request, call) => {
      if (call.path === '/api/me/media/uploads') {
        return json({
          media_id: MEDIA,
          state: 'PENDING',
          upload_path: `/api/me/media/${MEDIA}/bytes`,
          max_bytes: 10 * 1024 * 1024,
          expires_at: '2026-11-05T00:15:00Z',
          upload_method: 'PUT',
          revision: 1,
        })
      }
      if (call.path.endsWith('/bytes')) {
        // Hold the byte upload open so the test can cancel mid-flight.
        return new Promise<Response>(resolve => {
          releaseBytes = () => resolve(json({ media_id: MEDIA, state: 'UPLOADED', revision: 2 }))
        })
      }
      if (call.path.endsWith('/finalize')) return json(media())
      if (call.path === '/api/me/posts') return json(post())
      throw new Error(`unexpected ${call.path}`)
    })
    const value = composer(f)
    pick(value)
    value.edit({ description: 'cancelled' })
    const publishing = value.publishPost()
    await waitFor(() => expect(releaseBytes).not.toBeNull())
    value.cancelAll()
    releaseBytes!()
    const created = await publishing
    expect(created).toBeNull()
    // The upload never reached finalize and no post was created.
    expect(paths(f)).toEqual([
      'POST /api/me/media/uploads',
      `PUT /api/me/media/${MEDIA}/bytes`,
    ])
    expect(value.read().published).toBeNull()
    expect(value.read().phase).toBe('idle')
    expect(value.read().notice).toContain('cancelled')
    // The draft survives the cancellation.
    expect(value.read().draft.description).toBe('cancelled')
  })

  it('the upload reports its phase as cancelled and never ready', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    let releaseBytes: (() => void) | null = null
    f.handler((_request, call) => {
      if (call.path === '/api/me/media/uploads') {
        return json({
          media_id: MEDIA,
          state: 'PENDING',
          upload_path: `/api/me/media/${MEDIA}/bytes`,
          max_bytes: 10 * 1024 * 1024,
          expires_at: '2026-11-05T00:15:00Z',
          upload_method: 'PUT',
          revision: 1,
        })
      }
      if (call.path.endsWith('/bytes')) {
        return new Promise<Response>(resolve => {
          releaseBytes = () => resolve(json({ media_id: MEDIA, state: 'UPLOADED', revision: 2 }))
        })
      }
      throw new Error(`unexpected ${call.path}`)
    })
    const value = composer(f)
    pick(value)
    const publishing = value.publishPost()
    await waitFor(() => expect(releaseBytes).not.toBeNull())
    value.cancelAll()
    releaseBytes!()
    await publishing
    const states = value.uploadStates()
    expect(states[0]?.phase).toBe('cancelled')
    expect(states[0]?.media).toBeNull()
    expect(states[0]?.failure).toEqual({ kind: 'cancelled' })
    // The staged intent is reported so the caller can say it will be cleaned.
    expect(states[0]?.mediaId).toBe(MEDIA)
  })

  it('leaving the screen mid-upload cancels cleanly', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    let releaseBytes: (() => void) | null = null
    f.handler((_request, call) => {
      if (call.path === '/api/me/media/uploads') {
        return json({
          media_id: MEDIA,
          state: 'PENDING',
          upload_path: `/api/me/media/${MEDIA}/bytes`,
          max_bytes: 10 * 1024 * 1024,
          expires_at: '2026-11-05T00:15:00Z',
          upload_method: 'PUT',
          revision: 1,
        })
      }
      if (call.path.endsWith('/bytes')) {
        return new Promise<Response>(resolve => {
          releaseBytes = () => resolve(json({ media_id: MEDIA, state: 'UPLOADED', revision: 2 }))
        })
      }
      if (call.path.endsWith('/finalize')) return json(media())
      if (call.path === '/api/me/posts') return json(post())
      throw new Error(`unexpected ${call.path}`)
    })
    const value = composer(f)
    pick(value)
    const { unmount } = render(<ProfileMediaComposer composer={value} />)
    fireEvent.click(screen.getByRole('button', { name: 'Publish post' }))
    await waitFor(() => expect(releaseBytes).not.toBeNull())
    // The user navigates away: the component unmounts.
    unmount()
    releaseBytes!()
    await waitFor(() => expect(value.uploadStates()[0]?.phase).toBe('cancelled'))
    // Nothing was published after the screen was gone.
    expect(paths(f).some(path => path.endsWith('/finalize'))).toBe(false)
    expect(paths(f).some(path => path === 'POST /api/me/posts')).toBe(false)
  })

  it('removing an image cancels its upload before it can be published', async () => {
    const f = await setup()
    cleanups.push(f.dispose)
    uploadServer(f)
    const value = composer(f)
    pick(value)
    value.removeAttachment('local-photo.png')
    await value.publishPost()
    // With no attachment left, the publish is a text-only post: no upload ran.
    expect(paths(f)).toEqual(['POST /api/me/posts'])
    expect((f.calls[0]!.body as { media_ids: string[] }).media_ids).toEqual([])
  })
})

describe('the failure vocabulary', () => {
  it('maps a transport failure to a closed kind', () => {
    const live = new AbortController()
    expect(classifyFailure(new Error('boom'), live.signal)).toEqual({ kind: 'unknown', uncertain: false })
    const aborted = new AbortController()
    aborted.abort()
    expect(classifyFailure(new Error('boom'), aborted.signal)).toEqual({ kind: 'cancelled' })
  })

  it('retries only transient failures', () => {
    expect(retryable({ kind: 'unavailable', retryAfterMs: null })).toBe(true)
    expect(retryable({ kind: 'unknown', uncertain: true })).toBe(true)
    expect(retryable({ kind: 'cancelled' })).toBe(false)
    expect(retryable({ kind: 'too-large' })).toBe(false)
    expect(retryable({ kind: 'unsupported-type' })).toBe(false)
  })

  it('never puts a server diagnostic in a user-facing line', () => {
    for (const kind of ['cancelled', 'unsupported-type', 'too-large', 'invalid', 'unavailable', 'unknown'] as const) {
      const message = failureMessage(kind === 'unavailable' ? { kind, retryAfterMs: null } : kind === 'unknown' ? { kind, uncertain: true } : { kind })
      expect(message.length).toBeGreaterThan(0)
      expect(message).not.toContain('DEPENDENCY')
      expect(message).not.toContain('PROFILE_')
    }
  })

  it('the upload module never deletes a media row it does not own', () => {
    // There is no delete operation in the frozen profile media contract, so the
    // client cannot and must not try to remove a staged asset itself.
    const source = readFileSync(new FileURL('../upload.ts', import.meta.url), 'utf8')
    const code = source.replace(/\/\*[\s\S]*?\*\//g, '').split('\n')
      .filter(line => !line.trimStart().startsWith('//')).join('\n')
    expect(code).not.toContain("method: 'DELETE'")
    expect(code).not.toContain('deleteProfileMedia')
  })
})

/** A state helper so the type of an upload state is exercised. */
export const uploadPhase = (state: UploadState) => state.phase
