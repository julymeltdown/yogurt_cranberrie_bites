import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createReviewedCharacterRegistry } from '../../reviewed-registry'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import { deferred, pin, setup } from '../../../../features/avatar/__tests__/reviewed-production/fixtures'
import { object } from '../../../../features/auth/model/contract'

const cleanup: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.test' }))
afterEach(() => { cleanup.splice(0).reverse().forEach(fn => fn()); vi.unstubAllGlobals(); vi.restoreAllMocks() })
async function fixture() {
  const f = await setup(); cleanup.push(f.dispose)
  const bootstrap = await decodeBootstrap(f.bundle(), { generation: f.scope.generation, userId: f.scope.userId!, eventId: f.scope.eventId! })
  const registry = createReviewedCharacterRegistry(f.auth, 'https://mcr.test'); cleanup.push(registry.dispose)
  return { ...f, bootstrap, registry }
}
it('shares one pending admission and minted runtime across shell consumers without loading artwork or mutating profiles', async () => {
  const f = await fixture(), first = f.registry.admit(f.scope, f.bootstrap)
  expect(f.registry.admit(f.scope, structuredClone(f.bootstrap))).toBe(first)
  const owner = await first
  expect(owner).not.toBeNull(); expect(owner?.isCurrent()).toBe(true)
  expect(await f.registry.admit(f.scope, f.bootstrap)).toBe(owner)
  expect(f.registry.read(f.scope, pin)).toBe(owner)
  expect(f.calls).toEqual([{ path: `/api/concert/events/${f.scope.eventId}/bootstrap`, method: 'GET', body: null }])
  expect(owner?.runtime.diagnostics()).toMatchObject({ decodedAtlases: 0, leases: 0, clockListeners: 0 })
})
it.each(['policy', 'identity', 'dispose', 'clear'] as const)('fences pending admission across %s replacement', async kind => {
  const f = await fixture(), response = deferred<Response>()
  f.handler(() => response.promise)
  const pending = f.registry.admit(f.scope, f.bootstrap)
  await vi.waitFor(() => expect(f.calls).toHaveLength(1))
  if (kind === 'policy') f.auth.privacy.changePolicy(f.scope, 'block')
  if (kind === 'identity') f.auth.session.replace(f.scope.generation, null)
  if (kind === 'dispose') f.registry.dispose()
  if (kind === 'clear') f.registry.clear()
  response.resolve(Response.json(f.bundle()))
  expect(await pending).toBeNull(); expect(f.registry.read(f.scope, pin)).toBeNull()
})
it('withdraws authority, clocks and borrowed resources synchronously on policy change', async () => {
  const f = await fixture(), owner = (await f.registry.admit(f.scope, f.bootstrap))!
  owner.runtime.onFrame(() => {})
  f.auth.privacy.changePolicy(f.scope, 'block')
  expect(owner.isCurrent()).toBe(false); expect(owner.authority.signal.aborted).toBe(true)
  expect(owner.runtime.diagnostics()).toMatchObject({ disposed: true, leases: 0, clockListeners: 0 })
  expect(f.registry.read(f.scope, pin)).toBeNull()
})
it.each(['foreign participant', 'foreign event', 'changed manifest', 'static family'] as const)('rejects %s before protected HTTP', async kind => {
  const f = await fixture(), bootstrap = structuredClone(f.bootstrap)
  const other = '00000000-0000-4000-8000-000000000099'
  const bad = kind === 'foreign participant' ? { ...bootstrap, participant: { ...bootstrap.participant, userId: other } }
    : kind === 'foreign event' ? { ...bootstrap, event: { ...bootstrap.event, eventId: other } } : bootstrap
  if (kind === 'changed manifest') object(bootstrap.document!.avatar_manifest).content_hash = `sha256:${'0'.repeat(64)}`
  if (kind === 'static family') object(bootstrap.document!.avatar_manifest).representation = 'whole_character'
  expect(await f.registry.admit(f.scope, bad)).toBeNull(); expect(f.calls).toHaveLength(0)
})
it('rejects a fresh authenticated C03 that changed identity and can retry after an honest rejection', async () => {
  const f = await fixture(), original = structuredClone(f.bundle())
  object(f.bundle().participant).user_id = '00000000-0000-4000-8000-000000000099'
  expect(await f.registry.admit(f.scope, f.bootstrap)).toBeNull()
  f.setBundle(original)
  const owner = await f.registry.admit(f.scope, f.bootstrap)
  expect(owner?.isCurrent()).toBe(true); expect(f.calls).toHaveLength(2)
})
it('never lends a runtime for a foreign pin or privacy generation', async () => {
  const f = await fixture(); await f.registry.admit(f.scope, f.bootstrap)
  expect(f.registry.read(f.scope, { ...pin, manifest_version: 1 })).toBeNull()
  expect(f.registry.read({ ...f.scope, policyGeneration: f.scope.policyGeneration + 1 }, pin)).toBeNull()
  expect(f.registry.read({ ...f.scope, generation: f.scope.generation + 1 }, pin)).toBeNull()
})
it('replaces a disposed owner instead of returning an obsolete promise', async () => {
  const f = await fixture(), first = (await f.registry.admit(f.scope, f.bootstrap))!
  first.dispose()
  const next = await f.registry.admit(f.scope, f.bootstrap)
  expect(next).not.toBe(first); expect(next?.isCurrent()).toBe(true); expect(f.calls).toHaveLength(2)
})
it('closed registry does not start new admission', async () => {
  const f = await fixture(); f.registry.dispose()
  expect(await f.registry.admit(f.scope, f.bootstrap)).toBeNull(); expect(f.calls).toHaveLength(0)
})
