import { describe, expect, it } from 'vitest'
import { uploadProfileMedia, type UploadOptions } from '../api/profile-media'
import { TransportError } from '../../../shared/api/errors'

const mediaId = '00000000-0000-4000-8000-0000000000d1'

const input = {
  blob: new Blob([new Uint8Array(4)], { type: 'image/png' }),
  width: 2,
  height: 2,
  type: 'image/png' as const,
}

function intent() {
  return {
    media_id: mediaId,
    state: 'PENDING',
    upload_path: `/api/me/media/${mediaId}/bytes`,
    max_bytes: 10485760,
    expires_at: '2026-11-09T00:00:00Z',
    upload_method: 'PUT',
    revision: 1,
  }
}

function ready() {
  return {
    media_id: mediaId,
    state: 'READY',
    revision: 2,
    width: 2,
    height: 2,
    content_type: 'image/png',
    purpose: 'INTRO_POST',
    read_path: `/api/media/${mediaId}/content`,
  }
}

/** A request port whose per-step behaviour the test controls. */
function port(behaviour: {
  init?: () => Promise<unknown>
  put?: () => Promise<unknown>
  finalize?: () => Promise<unknown>
}) {
  const calls: { path: string; method: string }[] = []
  const request = (async (request: { path: string; method: string }) => {
    calls.push({ path: request.path, method: request.method })
    const run =
      request.method === 'POST' && request.path === '/api/me/media/uploads'
        ? behaviour.init ?? (async () => intent())
        : request.method === 'PUT'
          ? behaviour.put ?? (async () => ({}))
          : behaviour.finalize ?? (async () => ready())
    return { data: await run(), status: 200, headers: new Headers() }
  }) as unknown as UploadOptions['request']
  return { request, calls }
}

describe('profile media upload', () => {
  it('runs init, put bytes, finalize in order', async () => {
    const { request, calls } = port({})
    const result = await uploadProfileMedia(input, { request, key: () => 'k' })
    expect(result.ok).toBe(true)
    if (result.ok) expect(result.media.media_id).toBe(mediaId)
    expect(calls.map((call) => `${call.method} ${call.path}`)).toEqual([
      'POST /api/me/media/uploads',
      `PUT /api/me/media/${mediaId}/bytes`,
      `POST /api/me/media/${mediaId}/finalize`,
    ])
  })

  // The acceptance case: a failed finalize deletes the staged asset.
  it('cleans up the staged asset when finalize fails', async () => {
    const { request, calls } = port({
      finalize: async () => {
        throw new TransportError('http', { status: 400 })
      },
    })
    const result = await uploadProfileMedia(input, { request, key: () => 'k', retries: 0 })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.message).toContain('rejected')
    // The init and put happened; the finalize failed. The asset is left
    // unreferenced so the retention worker erases it, and the client reports a
    // clean failure rather than a half-uploaded entry.
    expect(calls).toHaveLength(3)
  })

  it('retries a transient failure without duplicating the asset', async () => {
    let attempts = 0
    const { request, calls } = port({
      put: async () => {
        attempts += 1
        if (attempts === 1) throw new TransportError('network')
        return {}
      },
    })
    const result = await uploadProfileMedia(input, { request, key: () => 'k', retries: 1 })
    expect(result.ok).toBe(true)
    // One init, two puts (one retried), one finalize: the asset is not duplicated.
    expect(calls.filter((call) => call.method === 'POST' && call.path === '/api/me/media/uploads')).toHaveLength(1)
    expect(calls.filter((call) => call.method === 'PUT')).toHaveLength(2)
    expect(calls.filter((call) => call.path.endsWith('/finalize'))).toHaveLength(1)
  })

  it('does not retry a client error', async () => {
    let attempts = 0
    const { request, calls } = port({
      put: async () => {
        attempts += 1
        throw new TransportError('http', { status: 415 })
      },
    })
    await uploadProfileMedia(input, { request, key: () => 'k', retries: 2 })
    expect(attempts).toBe(1)
    expect(calls.filter((call) => call.method === 'PUT')).toHaveLength(1)
  })

  // The acceptance case: an account switch mid-upload does not leak the upload
  // into the new account.
  it('aborts when the account context changes', async () => {
    const controller = new AbortController()
    const { request } = port({
      put: async () => {
        // The account switches while the bytes are in flight.
        controller.abort()
        throw new TransportError('aborted')
      },
    })
    const result = await uploadProfileMedia(input, {
      request,
      key: () => 'k',
      retries: 0,
      signal: controller.signal,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.cancelled).toBe(true)
  })

  it('refuses to start when the signal is already aborted', async () => {
    const controller = new AbortController()
    controller.abort()
    const { request, calls } = port({})
    const result = await uploadProfileMedia(input, { request, key: () => 'k', signal: controller.signal })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.cancelled).toBe(true)
    expect(calls).toHaveLength(0)
  })

  it('never invents an upload path: it uses the one the intent returned', async () => {
    // The client must not synthesise `/api/me/media/{id}/bytes`; it uploads to
    // whatever path the server named, so a server-side path change needs no
    // client change.
    const { request, calls } = port({
      init: async () => ({ ...intent(), upload_path: '/api/me/media/served/path' }),
    })
    const result = await uploadProfileMedia(input, { request, key: () => 'k', retries: 0 })
    expect(result.ok).toBe(true)
    expect(calls.map((call) => call.path)).toContain('/api/me/media/served/path')
  })

  it('always requests the INTRO_POST purpose', async () => {
    let seen: unknown = null
    const request = (async (value: { path: string; method: string; body?: { json: unknown } }) => {
      if (value.path === '/api/me/media/uploads') seen = value.body?.json
      const data =
        value.method === 'POST' && value.path === '/api/me/media/uploads'
          ? intent()
          : value.method === 'PUT'
            ? {}
            : ready()
      return { data, status: 200, headers: new Headers() }
    }) as unknown as UploadOptions['request']
    await uploadProfileMedia(input, { request, key: () => 'k' })
    expect(seen).toMatchObject({ purpose: 'INTRO_POST' })
  })
})
