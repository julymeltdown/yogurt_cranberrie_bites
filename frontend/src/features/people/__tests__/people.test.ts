// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { decodePage, decodeProfile } from '../contract'
import { deferred, eventId, harness, publicIntro } from './fixtures/harness'
const owned: ReturnType<typeof harness>[] = []
afterEach(() => owned.splice(0).forEach(h => h.dispose()))
async function setup() { const h = harness(); owned.push(h); h.controller.start(); await vi.waitFor(() => expect(h.controller.read().page.phase).toBe('ready')); return h }
describe('FE29 public discovery boundary', () => {
  it('browses without any X identity and filters only with frozen q/limit/cursor', async () => {
    const h = await setup(); expect(h.controller.read().page.data?.items).toHaveLength(1)
    h.controller.setSearch('show'); expect(h.requests).toHaveLength(1); expect(h.controller.setQuery(' show ')).toBe(true)
    await vi.waitFor(() => expect(h.requests).toHaveLength(2)); expect(new URL(h.requests[1]!.url).searchParams.get('q')).toBe('show')
    expect(h.controller.setQuery('a'.repeat(101))).toBe(false)
    expect(h.controller.setQuery('\u0000')).toBe(false)
  })
  it('rejects age, policy, claim and secret-handle fields in every public projection', async () => {
    const h = await setup(), row = publicIntro(), scope = h.privacy.capture()
    for (const field of ['age_bounds', 'age', 'claim', 'social_handles', 'instagram_handle']) {
      expect(() => decodePage({ items: [{ ...row, [field]: 'secret' }], next_cursor: null }, scope)).toThrow()
      expect(() => decodeProfile({ ...row.author, [field]: 'secret' }, row.author.user_id)).toThrow()
    }
    expect(() => decodePage({ items: [{ ...row, event_id: '00000000-0000-4000-8000-000000000999' }], next_cursor: null }, scope)).toThrow()
  })
  it.each([403, 404])('makes denied/direct ID %s opaque and evicts matching cached discovery', async status => {
    const h = await setup(), id = publicIntro().author.user_id
    h.respond(async () => Response.json({ code: 'GATEWAY', message: 'private reason', details: { reason: status === 403 ? 'PERMISSION_DENIED' : 'RESOURCE_UNAVAILABLE' } }, { status }))
    await h.controller.openProfile(id); expect(h.controller.read().profile).toMatchObject({ phase: 'error', failure: 'unavailable', data: null })
    expect(h.controller.read().page.data?.items).toHaveLength(0); expect(JSON.stringify(h.controller.read())).not.toContain('private reason')
  })
  it('drops stale search/profile responses across identity/event/policy and clears search text', async () => {
    const h = await setup(), pending = deferred<Response>(); h.respond(() => pending.promise)
    h.controller.setSearch('private search'); const response = h.controller.openProfile(publicIntro().author.user_id)
    h.session.replace(h.session.read().generation, { userId: '00000000-0000-4000-8000-000000000004', eventId, accessToken: 'other-token' })
    expect(h.controller.read().input).toBe(''); expect(h.controller.read().profile.data).toBeNull()
    pending.resolve(Response.json(publicIntro().author)); await response; expect(h.controller.read().profile.data).toBeNull()
    h.privacy.changePolicy(h.privacy.capture(), 'suspended'); expect(h.controller.read().page.data).toBeNull()
  })
  it('rejects repeated cursor/duplicates and never auto-paginates an empty page', async () => {
    const h = await setup(); h.respond(async () => Response.json({ items: [], next_cursor: 'next_cursor' }))
    await h.controller.load(); expect(h.controller.read().page.data?.items).toEqual([])
    const count = h.requests.length; await Promise.resolve(); expect(h.requests).toHaveLength(count)
    await h.controller.load(true); expect(h.controller.read().page.phase).toBe('error')
  })
})
