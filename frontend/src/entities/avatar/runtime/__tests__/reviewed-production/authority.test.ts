import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { manifest, pin, preset, rehash, setup, deferred } from '../../../../../features/avatar/__tests__/reviewed-production/fixtures'
import { createReviewedProductionAuthority, isReviewedProductionAuthority } from '../../motion-adoption/reviewed-production'
import { createMotionRuntime } from '../../motion-adoption/runtime'
import { createReviewedMotionFixtureAuthority, decodeReviewedMotionManifest, reviewedMotionAuthorityOperations } from '../../motion-adoption/reviewed-manifest'
import { object } from '../../../../../features/auth/model/contract'

const cleanup: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.test' }))
afterEach(() => { cleanup.splice(0).reverse().forEach(fn => fn()); vi.unstubAllGlobals(); vi.restoreAllMocks() })
async function fixture() { const f = await setup(); cleanup.push(f.dispose); return f }

it.each(manifest.presets)('mints only from protected validated C03 bootstrap for $preset_id', async row => {
  const f = await fixture(), bundle = f.bundle()
  bundle.avatar = { revision: 1, config: row.config }; object(object(bundle.participant).profile).avatar = bundle.avatar
  const authority = await f.create(); cleanup.push(authority.dispose)
  expect(isReviewedProductionAuthority(authority)).toBe(true)
  expect(authority.qualification).toBe('REVIEWED_PRODUCTION')
  expect(authority.decodeConfig(row.config)).toEqual(row.config)
  expect(authority.scope).toEqual(f.scope); expect(Object.isFrozen(authority)).toBe(true)
  expect(f.calls).toEqual([{ path: `/api/concert/events/${f.scope.eventId}/bootstrap`, method: 'GET', body: null }])
})
it('does not promote structural/fixture operations or a copied authority', async () => {
  const f = await fixture(), stage = await decodeReviewedMotionManifest(manifest, pin)
  const fixtureAuthority = createReviewedMotionFixtureAuthority(stage), operations = reviewedMotionAuthorityOperations(stage)
  expect(stage.qualification).toBe('STAGED_NOT_ADMITTED'); expect(fixtureAuthority.qualification).toBe('TEST_ONLY')
  expect(isReviewedProductionAuthority(fixtureAuthority)).toBe(false)
  expect(isReviewedProductionAuthority(operations)).toBe(false)
  const authority = await f.create(); cleanup.push(authority.dispose)
  expect(isReviewedProductionAuthority({ ...authority })).toBe(false)
  expect(() => createMotionRuntime({ authority: { ...authority }, scope: f.scope, privacy: f.auth.privacy, origin: 'https://mcr.test' })).toThrow()
})
it.each(['foreign participant', 'foreign event', 'private field', 'manifest hash', 'rehashed tamper', 'theme mismatch', 'legacy config', 'legacy family'])(
  'rejects malformed or unauthorized public bootstrap: %s', async change => {
    const f = await fixture(), b = f.bundle(), m = object(b.avatar_manifest)
    if (change === 'foreign participant') object(b.participant).user_id = '00000000-0000-4000-8000-000000000099'
    if (change === 'foreign event') object(b.event).event_id = '00000000-0000-4000-8000-000000000099'
    if (change === 'private field') object(object(b.participant).profile).email = 'private@example.invalid'
    if (change === 'manifest hash') m.content_hash = `sha256:${'0'.repeat(64)}`
    if (change === 'rehashed tamper') { object(m.atlas).path = 'https://untrusted.invalid/image.png'; await rehash(m) }
    if (change === 'theme mismatch') { const themes = b.theme_manifests; if (!Array.isArray(themes)) throw Error('themes'); object(themes[0]).avatar_manifest_version = 1; await rehash(object(themes[0])) }
    if (change === 'legacy config') { object(b.avatar).config = { ...preset(0), contract_version: '1.1.0' }; object(object(b.participant).profile).avatar = b.avatar }
    if (change === 'legacy family') { m.manifest_id = 'mcr_selected_motion_20261103'; await rehash(m) }
    await expect(f.create()).rejects.toThrow()
  })
it.each(['manifest_id', 'manifest_version', 'content_hash'] as const)('requires the Root requested exact %s pin', async key => {
  const f = await fixture(), bad = { ...pin, [key]: key === 'manifest_version' ? 1 : 'foreign' }
  await expect(createReviewedProductionAuthority({ auth: f.auth, scope: f.scope, pin: bad, signal: f.signal.signal })).rejects.toThrow()
})
it('rejects an already stale or aborted scope before any HTTP', async () => {
  const f = await fixture(); f.signal.abort()
  await expect(f.create()).rejects.toThrow(); expect(f.calls).toHaveLength(0)
  const g = await fixture(); g.auth.privacy.changePolicy(g.scope, 'block')
  await expect(g.create()).rejects.toThrow(); expect(g.calls).toHaveLength(0)
})
it.each(['abort', 'policy', 'identity'] as const)('cannot mint across an in-flight %s change', async kind => {
  const f = await fixture(), response = deferred<Response>()
  f.handler(() => response.promise)
  const pending = f.create().then(() => false, () => true)
  await vi.waitFor(() => expect(f.calls).toHaveLength(1))
  if (kind === 'abort') f.signal.abort()
  if (kind === 'policy') f.auth.privacy.changePolicy(f.scope, 'block')
  if (kind === 'identity') f.auth.session.replace(f.scope.generation, null)
  response.resolve(Response.json(f.bundle()))
  expect(await pending).toBe(true)
})
it('withdraws runtime clocks and authority synchronously on private policy change', async () => {
  const f = await fixture(), authority = await f.create()
  const runtime = createMotionRuntime({ authority, scope: f.scope, privacy: f.auth.privacy, origin: 'https://mcr.test' })
  cleanup.push(runtime.dispose)
  runtime.onFrame(() => {}); expect(runtime.diagnostics().clockListeners).toBe(1)
  f.auth.privacy.changePolicy(f.scope, 'block')
  expect(authority.signal.aborted).toBe(true); expect(authority.isCurrent()).toBe(false)
  expect(runtime.isCurrent()).toBe(false)
  expect(runtime.diagnostics()).toMatchObject({ disposed: true, leases: 0, clockListeners: 0, decodedAtlases: 0 })
  expect(() => runtime.validateConfig(preset(0))).toThrow()
})
it('forbids bypass loaders and foreign privacy scope on a genuine public authority', async () => {
  const f = await fixture(), authority = await f.create(); cleanup.push(authority.dispose)
  expect(() => createMotionRuntime({ authority, scope: f.scope, privacy: f.auth.privacy, origin: 'https://mcr.test', load: async () => { throw Error('Bypass') } })).toThrow()
  expect(() => createMotionRuntime({ authority, scope: { ...f.scope, policyGeneration: f.scope.policyGeneration + 1 }, privacy: f.auth.privacy, origin: 'https://mcr.test' })).toThrow()
})
