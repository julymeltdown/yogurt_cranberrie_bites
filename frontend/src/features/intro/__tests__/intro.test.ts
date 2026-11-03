// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createIntroController } from '../controller'
import { boundsSchema, fromIntro, withinBounds, type OwnIntro } from '../contract'
import { deferred, eventId, harness, introId, userId } from '../../people/__tests__/fixtures/harness'
const owned: (() => void)[] = []
afterEach(() => owned.splice(0).forEach(cleanup => cleanup()))
const now = Date.parse('2026-11-07T12:00:00Z')
function own(): OwnIntro { return { intro_id: introId, event_id: eventId, owner_id: userId, text: 'Hello community', age_bounds: { minimum_age: 20, maximum_age_exclusive: 30 }, expires_at: null, created_at: '2026-11-07T08:00:00Z', state: 'DRAFT', revision: 1 } }
function error(status: number, reason: string) { return Response.json({ code: 'GATEWAY', message: 'private server detail', details: { reason } }, { status }) }
async function setup() {
  const h = harness(), controller = createIntroController({ ...h.options, now: () => now }); let row = own()
  h.respond(async request => { if (request.method !== 'GET') { const body = await request.json(); row = { ...row, ...body, revision: row.revision + 1 }; delete (row as unknown as Record<string, unknown>).expected_revision }; return Response.json(row) })
  owned.push(() => { controller.dispose(); h.dispose() }); controller.start(); await vi.waitFor(() => expect(controller.read().load.phase).toBe('ready'))
  return { h, controller, row: () => row }
}
function receipt(key: string, state = 'SUCCEEDED') { return { receipt_id: '00000000-0000-4000-8000-000000000046', event_id: eventId, actor_id: userId, operation_id: 'setConcertIntro', idempotency_key: key, state, resource_id: state === 'SUCCEEDED' ? introId : null, resource_revision: state === 'SUCCEEDED' ? 2 : null, failure_reason: null, result_status: state === 'SUCCEEDED' ? 200 : null, created_at: '2026-11-07T08:00:00Z', expires_at: '2026-11-08T08:00:00Z' } }
describe('FE30 owner introduction', () => {
  it('enforces half-open form bounds including minimum inclusive and maximum exclusive', () => {
    const b = { minimum_age: 20, maximum_age_exclusive: 30 }; expect(withinBounds(b, 20)).toBe(true); expect(withinBounds(b, 29)).toBe(true); expect(withinBounds(b, 30)).toBe(false); expect(withinBounds(b, 19)).toBe(false)
    for (const value of [{ minimum_age: null, maximum_age_exclusive: 30 }, { minimum_age: 20, maximum_age_exclusive: null }, { minimum_age: 30, maximum_age_exclusive: 30 }, { minimum_age: 13, maximum_age_exclusive: 20 }]) expect(boundsSchema.safeParse(value).success).toBe(false)
    expect(boundsSchema.safeParse({ minimum_age: null, maximum_age_exclusive: null }).success).toBe(true)
  })
  it('saves private conditions with original revision and key, then publishes only a saved draft', async () => {
    const { h, controller } = await setup(); controller.edit({ ...controller.read().draft, text: 'Updated text' }); expect(await controller.transition('publish')).toBe(false)
    expect(await controller.save()).toBe(true); const request = h.requests[1]!; expect(request.method).toBe('PUT'); expect(await request.json()).toMatchObject({ expected_revision: 1, text: 'Updated text', age_bounds: { minimum_age: 20, maximum_age_exclusive: 30 } }); expect(request.headers.get('idempotency-key')).toMatch(/^[0-9a-f-]{36}$/)
    h.respond(async () => Response.json({ ...controller.read().saved, state: 'PUBLISHED', revision: 3 })); expect(await controller.transition('publish')).toBe(true); expect(controller.read().saved?.state).toBe('PUBLISHED')
  })
  it('keeps local read-only draft and permits explicit safety close without discarding it', async () => {
    const { h, controller } = await setup(); h.access({ readOnly: true }); controller.edit({ ...controller.read().draft, text: 'Private unfinished draft' }); const count = h.requests.length
    expect(await controller.save()).toBe(false); expect(await controller.transition('publish')).toBe(false); expect(h.requests).toHaveLength(count)
    h.respond(async () => Response.json({ ...own(), state: 'CLOSED', revision: 2 })); expect(await controller.transition('close')).toBe(true)
    expect(controller.read().saved?.state).toBe('CLOSED'); expect(controller.read().draft.text).toBe('Private unfinished draft')
  })
  it('preserves conflicts and requires explicit latest-version review', async () => {
    const { h, controller } = await setup(); controller.edit({ ...controller.read().draft, text: 'My draft' }); h.respond(async () => error(409, 'VERSION_CONFLICT')); expect(await controller.save()).toBe(false)
    expect(controller.read()).toMatchObject({ needsReview: true, draft: { text: 'My draft' }, command: { phase: 'conflict', locked: false } })
    h.respond(async () => Response.json({ ...own(), text: 'Other session', revision: 2 })); await controller.load(); expect(controller.read().draft.text).toBe('My draft'); expect(controller.read().latest?.revision).toBe(2)
    controller.reconcile('keep-draft'); expect(controller.read().saved?.revision).toBe(2); expect(controller.read().needsReview).toBe(false); expect(controller.read().draft.text).toBe('My draft')
  })
  it.each([404, 503])('never replays unknown writes or unlocks after command status %s', async status => {
    const { h, controller } = await setup(); controller.edit({ ...controller.read().draft, text: 'Keep this' }); h.respond(async () => error(503, 'SERVICE_UNAVAILABLE'))
    expect(await controller.save()).toBe(false); expect(controller.read().command.phase).toBe('uncertain'); const key = h.requests.at(-1)!.headers.get('idempotency-key'), count = h.requests.length
    expect(await controller.save()).toBe(false); expect(h.requests).toHaveLength(count)
    h.respond(async () => error(status, status === 404 ? 'RESOURCE_UNAVAILABLE' : 'SERVICE_UNAVAILABLE')); await controller.checkStatus(); const check = new URL(h.requests.at(-1)!.url); expect(check.searchParams.get('idempotency_key')).toBe(key); expect(check.searchParams.get('operation_id')).toBe('setConcertIntro'); expect(controller.read().command.locked).toBe(true); expect(controller.read().draft.text).toBe('Keep this')
  })
  it('reads authorized resource after successful receipt and never treats metadata as saved content', async () => {
    const { h, controller } = await setup(); controller.edit({ ...controller.read().draft, text: 'Committed text' }); let key = '', reads = 0
    h.respond(async request => { if (request.method === 'PUT') { key = request.headers.get('idempotency-key')!; return Response.json(receipt(key, 'PENDING'), { status: 202 }) }; if (new URL(request.url).pathname.endsWith('/commands')) return Response.json(receipt(key)); reads++; return Response.json({ ...own(), text: 'Committed text', revision: 2 }) })
    expect(await controller.save()).toBe(true); expect(controller.read().command.phase).toBe('pending'); expect(controller.read().saved?.revision).toBe(1)
    await controller.checkStatus(); expect(reads).toBe(1); expect(controller.read().saved?.revision).toBe(2); expect(controller.read().command).toMatchObject({ phase: 'succeeded', locked: false }); expect(h.requests.filter(r => r.method === 'PUT')).toHaveLength(1)
  })
  it.each(['RETIRED', 'wrong-actor', 'stale-read'])('keeps %s outcomes locked', async variant => {
    const { h, controller } = await setup(); let key = ''; h.respond(async request => { if (request.method === 'PUT') { key = request.headers.get('idempotency-key')!; return Response.json(receipt(key, 'PENDING'), { status: 202 }) }; if (new URL(request.url).pathname.endsWith('/commands')) return Response.json({ ...receipt(key, variant === 'RETIRED' ? 'RETIRED' : 'SUCCEEDED'), ...(variant === 'wrong-actor' ? { actor_id: '00000000-0000-4000-8000-000000000099' } : {}) }); return Response.json(own()) })
    await controller.save(); await controller.checkStatus(); expect(controller.read().command.locked).toBe(true); expect(controller.read().saved?.revision).toBe(1)
  })
  it('erases private drafts synchronously and ignores late accepted responses across identity change', async () => {
    const { h, controller } = await setup(), pending = deferred<Response>(); controller.edit({ ...controller.read().draft, text: 'Private text' }); h.respond(() => pending.promise); const save = controller.save()
    h.session.replace(h.session.read().generation, { userId: '00000000-0000-4000-8000-000000000004', eventId, accessToken: 'other' }); expect(controller.read().draft.text).toBe(''); expect(controller.read().saved).toBeNull()
    pending.resolve(Response.json({ ...own(), text: 'Private text', revision: 2 })); await save; expect(controller.read().saved).toBeNull(); expect(controller.read().command.phase).not.toBe('succeeded')
  })
  it('retains the original uncertain key across event-access interruption and recovers explicitly', async () => {
    const { h, controller } = await setup(), late = deferred<Response>(); let key = ''
    h.respond(async request => { key = request.headers.get('idempotency-key')!; return late.promise }); const save = controller.save(); await vi.waitFor(() => expect(key).not.toBe(''))
    h.access({ available: false }); expect(controller.read().command).toMatchObject({ phase: 'uncertain', locked: true }); late.resolve(Response.json({ ...own(), revision: 2 })); await save
    h.access({ available: true }); h.respond(async request => new URL(request.url).pathname.endsWith('/commands') ? Response.json(receipt(key)) : Response.json({ ...own(), revision: 2 }))
    await controller.checkStatus(); expect(controller.read().command.phase).toBe('succeeded'); expect(h.requests.filter(r => r.method === 'PUT')).toHaveLength(1)
  })
  it('isolates throwing observers and fences reentrant identity changes at command acceptance', async () => {
    const { h, controller } = await setup(); controller.subscribe(() => { throw new Error('observer') }); let switched = false
    controller.subscribe(() => { if (!switched && controller.read().saved?.revision === 2) { switched = true; h.privacy.changePolicy(h.privacy.capture(), 'suspended') } })
    controller.edit(fromIntro(own())); await controller.save(); expect(controller.read().saved).toBeNull(); expect(controller.read().command.phase).not.toBe('succeeded')
  })
  it('rejects past expiry and one-sided bounds before issuing any command', async () => {
    const { h, controller } = await setup(); const count = h.requests.length; controller.edit({ ...controller.read().draft, expires_at: '2026-11-01T00:00:00Z' }); expect(await controller.save()).toBe(false)
    controller.edit({ ...controller.read().draft, expires_at: null, age_bounds: { minimum_age: 20, maximum_age_exclusive: null } as never }); expect(await controller.save()).toBe(false); expect(h.requests).toHaveLength(count)
  })
})
