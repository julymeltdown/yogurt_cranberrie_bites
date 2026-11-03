import { describe, expect, it, vi } from 'vitest'
import { TransportError } from '../../../../shared/api/errors'
import type { MediaCommandResult, UploadIntent } from '../../../../shared/media/contract'
import { deferred, event, file, image, intent, mediaId, ready, receipt, setup, uid, user } from './fixtures'

describe('image controller lifecycle', () => {
  it('requires explicit preparation/upload and only exposes READY after finalize with scope and revision checks', async () => {
    const h = setup(); const changed = vi.fn(), unsubscribe = h.controller.subscribe(changed)
    await h.controller.select(file()); expect(h.controller.read()).toMatchObject({ phase: 'prepared', attachment: null, scope: { userId: user, eventId: event, generation: 1 } })
    expect(h.transport.initialize).not.toHaveBeenCalled()
    await Promise.all([h.controller.upload(), h.controller.upload()])
    expect(h.controller.read()).toMatchObject({ phase: 'ready', attachment: ready() })
    expect(h.transport.initialize).toHaveBeenCalledTimes(1); expect(h.transport.finalize).toHaveBeenCalledTimes(1)
    expect(changed).toHaveBeenCalled(); unsubscribe(); h.controller.cancel(); expect(h.previews.revoke).toHaveBeenCalledTimes(1)
    expect(h.controller.read()).toMatchObject({ phase: 'cancelled', scope: null, attachment: null, previewUrl: null })
  })
  it('publishes measured current-stage progress and supports unknown upload progress', async () => {
    const values: (number | null)[] = []
    const h = setup({ upload: vi.fn(async (_b, _i, _f, _s, progress) => { progress(null); progress(40); progress(200); return { kind: 'result' as const, value: { media_id: mediaId, state: 'UPLOADED' as const, revision: 2 } } }) },
      async (_f, o) => { o.onProgress(20); o.onProgress(Number.NaN); return image() })
    h.controller.subscribe(() => values.push(h.controller.read().progress))
    await h.controller.select(file()); await h.controller.upload()
    expect(values).toContain(20); expect(values).toContain(40); expect(values).toContain(null); expect(values).not.toContain(200)
  })
  it.each(['preparing', 'initializing', 'uploading', 'finalizing'] as const)('cancel during %s settles promptly and fences noncooperative late completion', async stage => {
    const blocked = deferred<unknown>(); let aborted: AbortSignal | undefined
    const h = setup({
      ...(stage === 'initializing' ? { initialize: vi.fn(async (_b, _p, _i, s) => { aborted = s; return await blocked.promise as MediaCommandResult<UploadIntent> }) } : {}),
      ...(stage === 'uploading' ? { upload: vi.fn(async (_b, _i, _f, s) => { aborted = s; await blocked.promise; return { kind: 'result' as const, value: { media_id: mediaId, state: 'UPLOADED' as const, revision: 2 } } }) } : {}),
      ...(stage === 'finalizing' ? { finalize: vi.fn(async (_b, _p, _r, s) => { aborted = s; await blocked.promise; return { kind: 'result' as const, value: ready() } }) } : {}),
    }, stage === 'preparing' ? async (_f, o) => { aborted = o.signal; await blocked.promise; return image() } : undefined)
    let running: Promise<void>
    if (stage === 'preparing') running = h.controller.select(file())
    else { await h.controller.select(file()); running = h.controller.upload() }
    await vi.waitFor(() => { expect(h.controller.read().phase).toBe(stage); expect(aborted).toBeDefined() })
    h.controller.cancel(); await running
    expect(aborted?.aborted).toBe(true); expect(h.controller.read().attachment).toBeNull()
    blocked.resolve({ kind: 'pending', receipt: {} }); await Promise.resolve(); await Promise.resolve()
    expect(h.controller.read().phase).toBe('cancelled'); expect(h.controller.read().scope).toBeNull()
    expect(h.previews.revoke).toHaveBeenCalledTimes(stage === 'preparing' ? 0 : 1)
  })
  it.each(['identity', 'event', 'logout', 'policy', 'dispose'] as const)('synchronously clears private state after %s even if finalization resolves late', async mode => {
    const blocked = deferred<unknown>()
    const h = setup({ finalize: vi.fn(async () => { await blocked.promise; return { kind: 'result' as const, value: ready() } }) })
    await h.controller.select(file()); const pending = h.controller.upload()
    await vi.waitFor(() => expect(h.transport.finalize).toHaveBeenCalled())
    if (mode === 'policy') h.controller.cancel()
    else if (mode === 'dispose') h.controller.dispose()
    else h.session.replace(1, mode === 'logout' ? null : { userId: mode === 'identity' ? uid(8) : user, eventId: mode === 'event' ? uid(9) : event, accessToken: 'other' })
    expect(h.controller.read()).toMatchObject({ scope: null, previewUrl: null, attachment: null })
    await pending; blocked.resolve(true); await Promise.resolve()
    expect(h.controller.read().phase).toBe('cancelled'); expect(h.previews.revoke).toHaveBeenCalledTimes(1)
  })
  it('failed and unknown finalizations never attach; retry never replays mutation or mints new key', async () => {
    for (const error of [new TransportError('http', { status: 415 }), new TransportError('network', { uncertain: true })]) {
      const h = setup({ finalize: vi.fn(async () => { throw error }) })
      await h.controller.select(file()); await h.controller.upload()
      expect(h.controller.read()).toMatchObject({ phase: error.uncertain ? 'uncertain' : 'failed', attachment: null })
      await h.controller.retry(); await h.controller.upload(); expect(h.transport.initialize).toHaveBeenCalledTimes(1); expect(h.transport.finalize).toHaveBeenCalledTimes(1)
    }
  })
  it('allows only explicit local preparation retry before any mutation', async () => {
    const prepare = vi.fn().mockRejectedValueOnce(new Error('decode')).mockResolvedValueOnce(image())
    const h = setup({}, prepare); await h.controller.select(file()); expect(h.controller.read().phase).toBe('failed')
    await h.controller.retry(); expect(h.controller.read().phase).toBe('prepared'); expect(prepare).toHaveBeenCalledTimes(2); expect(h.transport.initialize).not.toHaveBeenCalled()
  })
  it.each(['init', 'bytes', 'finalize'] as const)('202 on %s stays uncertain; bounded recovery never replays', async stage => {
    const h = setup({
      ...(stage === 'init' ? { initialize: vi.fn(async b => ({ kind: 'pending' as const, receipt: receipt(b) })) } : {}),
      ...(stage === 'bytes' ? { upload: vi.fn(async b => ({ kind: 'pending' as const, receipt: receipt(b) })) } : {}),
      ...(stage === 'finalize' ? { finalize: vi.fn(async b => ({ kind: 'pending' as const, receipt: receipt(b) })) } : {}),
    })
    await h.controller.select(file()); await h.controller.upload()
    expect(h.controller.read()).toMatchObject({ phase: 'uncertain', error: 'command-pending', attachment: null })
    await h.controller.recover(); expect(h.transport.status).toHaveBeenCalledTimes(1); expect(h.transport.initialize).toHaveBeenCalledTimes(1)
    expect(h.controller.read().phase).toBe('uncertain')
  })
  it('successful receipt requires fresh authorized READY media before recovery succeeds', async () => {
    const h = setup({ finalize: vi.fn(async () => { throw new TransportError('timeout', { uncertain: true }) }),
      status: vi.fn(async b => receipt(b, { state: 'SUCCEEDED', resource_id: mediaId, resource_revision: 3, result_status: 200 })) })
    await h.controller.select(file()); await h.controller.upload(); await h.controller.recover()
    expect(h.transport.read).toHaveBeenCalledTimes(1); expect(h.controller.read()).toMatchObject({ phase: 'ready', attachment: ready() })
  })
  it.each(['unknown', 'retired', 'failed', 'expired', 'foreign', 'wrong-result-status', 'not-ready', 'wrong-purpose', 'wrong-resource', 'stale-resource'] as const)('fails closed during %s recovery without mutation replay', async mode => {
    const h = setup({ finalize: vi.fn(async () => { throw new Error('lost response') }), status: vi.fn(async b => {
      if (mode === 'unknown') throw new TransportError('http', { status: 404 })
      return receipt(b, mode === 'retired' ? { state: 'RETIRED' } : mode === 'failed' ? { state: 'FAILED', failure_reason: 'INVALID_MEDIA_TYPE' }
        : mode === 'expired' ? { created_at: '2026-11-03T00:00:00Z', expires_at: '2026-11-03T01:00:00Z' }
          : { state: 'SUCCEEDED', resource_id: mode === 'foreign' ? uid(99) : mediaId, resource_revision: 3, result_status: mode === 'wrong-result-status' ? 202 : 200 })
    }), read: vi.fn(async () => ({ ...ready(), ...(mode === 'not-ready' ? { state: 'UPLOADED' } : mode === 'wrong-purpose' ? { purpose: 'REPORT' } : mode === 'wrong-resource' ? { media_id: uid(99) } : mode === 'stale-resource' ? { revision: 2 } : {}) }) as ReturnType<typeof ready>) })
    await h.controller.select(file()); await h.controller.upload(); await h.controller.recover(); await h.controller.retry()
    expect(h.controller.read().attachment).toBeNull(); expect(h.controller.read().phase).toBe(mode === 'failed' ? 'failed' : 'uncertain')
    expect(h.transport.initialize).toHaveBeenCalledTimes(1); expect(h.transport.finalize).toHaveBeenCalledTimes(1)
  })
  it('revokes replaced previews once and final dispose is idempotent', async () => {
    const h = setup(); await h.controller.select(file()); await h.controller.select(file()); expect(h.previews.revoke).toHaveBeenCalledTimes(1)
    h.controller.dispose(); h.controller.dispose(); expect(h.previews.revoke).toHaveBeenCalledTimes(2)
    await h.controller.select(file()); expect(h.controller.read().scope).toBeNull()
  })
  it('rejects forged compressed outcomes before creating preview or initializing', async () => {
    const h = setup({}, async () => ({ ...image(), width: 4000 }))
    await h.controller.select(file()); expect(h.controller.read().phase).toBe('failed'); expect(h.previews.create).not.toHaveBeenCalled(); expect(h.transport.initialize).not.toHaveBeenCalled()
  })
  it('does not send when a state subscriber cancels at the next mutation boundary', async () => {
    const h = setup(); h.controller.subscribe(() => { if (h.controller.read().phase === 'initializing') h.controller.cancel() })
    await h.controller.select(file()); await h.controller.upload(); expect(h.transport.initialize).not.toHaveBeenCalled(); expect(h.controller.read().phase).toBe('cancelled')
  })
  it('rejects expired intent and untrusted target instead of issuing byte PUT', async () => {
    const h = setup({ initialize: vi.fn(async () => ({ kind: 'result' as const, value: { ...intent(), upload_path: 'https://evil.invalid/bytes' } })) })
    await h.controller.select(file()); await h.controller.upload(); expect(h.transport.upload).not.toHaveBeenCalled(); expect(h.controller.read().attachment).toBeNull()
  })
})
