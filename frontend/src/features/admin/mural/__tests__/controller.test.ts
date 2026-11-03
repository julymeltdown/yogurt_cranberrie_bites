import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fixture, ids, wall } from './fixture'
let h: ReturnType<typeof fixture>
beforeEach(() => { vi.stubGlobal('location', new URL('https://mcr.test')); h = fixture() })
afterEach(() => { h.cleanup(); vi.unstubAllGlobals() })
const control = (action: 'PAUSE' | 'RESUME' | 'FREEZE', revision = h.mural.read().mural!.revision) => ({ operation: 'controlConcertMural' as const, body: { action, expected_revision: revision, reason: 'Reviewed operation' } })
describe('FE39 wall control', () => {
  it('preview/cancel never POST; exact pause/resume/freeze commands are explicit, versioned and terminal', async () => {
    await h.mural.start(); expect(h.mural.prepare(control('PAUSE'))).toBe(true); h.mural.cancel(); expect(h.posts()).toHaveLength(0)
    for (const action of ['PAUSE', 'RESUME', 'FREEZE'] as const) { expect(h.mural.prepare(control(action))).toBe(true); await h.mural.confirm() }
    expect(h.mural.read().mural).toMatchObject({ state: 'FROZEN', revision: 13 }); expect(h.posts()).toHaveLength(3)
    expect(h.mural.prepare(control('RESUME'))).toBe(false)
    expect(await h.posts()[0]!.clone().json()).toEqual({ action: 'PAUSE', expected_revision: 10, reason: 'Reviewed operation' })
    expect(h.posts()[0]!.headers.get('X-MCR-Intent')).toBe('browser'); expect(h.posts()[0]!.cache).toBe('no-store')
    expect(new Set(h.posts().map(r => r.headers.get('Idempotency-Key'))).size).toBe(3)
  })
  it('restore requires PAUSED and binds the exact named snapshot into the confirmed body', async () => {
    await h.mural.start()
    const restore = { operation: 'restoreConcertMural' as const, body: { expected_revision: 10, snapshot_id: ids.restore, reason: 'Restore reviewed content' } }
    expect(h.mural.prepare(restore)).toBe(false)
    h.server.wall = { ...wall(), state: 'PAUSED', revision: 11 }; await h.mural.load()
    const pausedRestore = { ...restore, body: { ...restore.body, expected_revision: 11 } }; expect(h.mural.prepare(pausedRestore)).toBe(true)
    expect(h.mural.read().draft).toEqual(pausedRestore); await h.mural.confirm()
    expect(await h.posts()[0]!.clone().json()).toEqual(pausedRestore.body); expect(h.mural.read().mural).toMatchObject({ revision: 12, state: 'PAUSED' })
  })
  it('locks/unlocks half-open bounded rectangles and rejects empty/out-of-bounds previews', async () => {
    await h.mural.start()
    const body = { expected_revision: 10, x_min: 0, y_min: 3, x_max_exclusive: 128, y_max_exclusive: 4, locked: true, reason: 'Protect region' }
    expect(h.mural.prepare({ operation: 'lockConcertMuralRegion', body: { ...body, x_max_exclusive: 0 } })).toBe(false)
    expect(h.mural.prepare({ operation: 'lockConcertMuralRegion', body: { ...body, y_min: 128 } })).toBe(false)
    expect(h.mural.prepare({ operation: 'lockConcertMuralRegion', body })).toBe(true); await h.mural.confirm()
    expect(h.mural.read().mural!.locks).toEqual([{ x_min: 0, y_min: 3, x_max_exclusive: 128, y_max_exclusive: 4 }])
    expect(h.mural.prepare({ operation: 'lockConcertMuralRegion', body: { ...body, expected_revision: 11, locked: false } })).toBe(true); await h.mural.confirm()
    expect(h.mural.read().mural!.locks).toEqual([])
  })
  it.each(['forbidden', 'conflict', 'throttle'])('server %s clears preview and forces reread; client-hidden controls never authorize a write', async mode => {
    await h.mural.start(); h.server.mode = mode; h.mural.prepare(control('PAUSE')); await h.mural.confirm()
    expect(h.mural.read()).toMatchObject({ commandPhase: 'rejected', mural: null, draft: null, phase: 'error' }); expect(h.posts()).toHaveLength(1)
  })
  it.each(['lost', 'pending', 'html'])('%s result retains original key and blocks all new mutation; read/status never replays', async mode => {
    await h.mural.start(); h.server.mode = mode; h.mural.prepare(control('PAUSE')); await h.mural.confirm()
    const command = h.mural.read().command!; expect(command).not.toBeNull()
    await h.mural.load(); expect(h.mural.prepare(control('FREEZE'))).toBe(false); await h.mural.confirm()
    await h.mural.check(); expect(h.posts()).toHaveLength(1)
    if (mode === 'lost') expect(h.mural.read()).toMatchObject({ command: null, commandPhase: 'confirmed', mural: { state: 'PAUSED', revision: 11 } })
    else expect(h.mural.read().command?.key).toBe(command.key)
  })
  it('status metadata without an authorized result reread does not resolve unknown', async () => {
    await h.mural.start(); h.server.mode = 'lost'; h.mural.prepare(control('PAUSE')); await h.mural.confirm()
    h.server.transform = (value, request) => request.url.endsWith('/admin/mural') ? { ...wall(), revision: 10 } : value
    await h.mural.check(); expect(h.mural.read().commandPhase).toBe('unknown'); expect(h.posts()).toHaveLength(1)
  })
  it.each(['actor', 'event', 'role', 'policy'])('%s change scrubs private data/draft/command', async change => {
    await h.mural.start(); h.server.mode = 'lost'; h.mural.prepare(control('PAUSE')); await h.mural.confirm()
    if (change === 'actor') h.session.replace(h.session.read().generation, { userId: ids.restore, eventId: ids.event, accessToken: 'new_actor' })
    if (change === 'event') h.session.replace(h.session.read().generation, { userId: ids.actor, eventId: ids.restore, accessToken: 'new_event' })
    if (change === 'role') h.update({ ...h.context(), roleRevision: 2 })
    if (change === 'policy') h.privacy.changePolicy(h.privacy.capture(), 'suspended')
    expect(h.mural.read()).toMatchObject({ mural: null, draft: null, command: null, phase: 'unavailable' })
  })
  it('event revision changes invalidate preview and fence a delayed response', async () => {
    await h.mural.start(); h.mural.prepare(control('PAUSE'))
    h.update({ ...h.context(), eventRevision: 9 }); expect(h.mural.read().draft).toBeNull()
    let resolve!: (r: Response) => void
    h.server.intercept = () => new Promise(r => { resolve = r }); const pending = h.mural.load()
    await vi.waitFor(() => expect(resolve).toBeTypeOf('function'))
    h.update({ ...h.context(), eventRevision: 10 }); resolve(Response.json(wall())); await pending
    expect(h.mural.read().mural).toBeNull(); expect(h.posts()).toHaveLength(0)
  })
  it('missing/forged role context makes no requests; fresh server denial is still enforced', async () => {
    h.update(null); await h.mural.start(); expect(h.server.requests).toHaveLength(0)
    h.update({ ...h.context(), scope: h.privacy.capture(), role: 'OPERATOR', roleRevision: 1, eventRevision: 8, phase: 'LIVE' }); h.server.operator = false
    await h.mural.load(); expect(h.mural.read().phase).toBe('error'); expect(h.posts()).toHaveLength(0)
  })
  it('double confirm uses one immutable command; role revocation aborts a hung POST', async () => {
    await h.mural.start(); h.mural.prepare(control('PAUSE'))
    h.server.intercept = request => request.method === 'POST' ? new Promise(() => {}) : null
    const pending = h.mural.confirm(); await h.mural.confirm(); await vi.waitFor(() => expect(h.posts()).toHaveLength(1))
    h.update(null); await pending; expect(h.mural.read().command).toBeNull()
  })
})
