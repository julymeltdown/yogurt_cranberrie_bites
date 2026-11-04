import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createReviewedMotionPersistence } from '../../motion-adoption/production-persistence'
import { createReviewedProductionAuthority } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-production'
import { decodeReviewedMotionConfig } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-config'
import { TransportError } from '../../../../shared/api/errors'
import { object } from '../../../auth/model/contract'
import { deferred, layered, manifest, preset, setup } from './fixtures'

const cleanup: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.test' }))
afterEach(() => { cleanup.splice(0).reverse().forEach(fn => fn()); vi.unstubAllGlobals(); vi.restoreAllMocks() })
async function fixture() {
  const f = await setup(); cleanup.push(f.dispose)
  const authority = await f.create(); cleanup.push(authority.dispose)
  const persistence = createReviewedMotionPersistence({ auth: f.auth, authority }); cleanup.push(persistence.dispose)
  return { ...f, authority, persistence }
}
it('uses canonical protected GET and explicit PUT only, returning verified revision/config without private profile data', async () => {
  const f = await fixture(), signal = f.signal.signal
  expect(await f.persistence.read(signal)).toEqual({ revision: 1, config: preset(0) })
  expect(f.calls.filter(c => c.method === 'PUT')).toHaveLength(0)
  expect(await f.persistence.save(1, preset(4), signal)).toEqual({ revision: 2, config: preset(4) })
  expect(f.calls.at(-1)).toEqual({ path: `/api/concert/events/${f.scope.eventId}/me/avatar`, method: 'PUT',
    body: { expected_revision: 1, config: preset(4) } })
  expect(await f.persistence.read(signal)).toEqual(f.server())
})
it.each(manifest.presets)('saves and re-reads every closed preset $preset_id', async row => {
  const f = await fixture(); await f.persistence.read(f.signal.signal)
  const config = decodeReviewedMotionConfig(row.config)
  expect(await f.persistence.save(1, config, f.signal.signal)).toEqual({ revision: 2, config })
  expect((await f.persistence.read(f.signal.signal)).config).toEqual(config)
})
it('persists every independent admitted item, preserving all unrelated coordinates and nullable choices', async () => {
  const f = await fixture(); let config = layered(), revision = 1
  if (config.representation !== 'motion_layered') throw Error('Expected layered')
  await f.persistence.read(f.signal.signal)
  for (const item of manifest.items) {
    const before = config.item_ids
    config = decodeReviewedMotionConfig({ ...config, item_ids: { ...before, [item.slot]: item.item_id } })
    const saved = await f.persistence.save(revision++, config, f.signal.signal)
    expect(saved.config).toEqual(config)
    if (saved.config.representation !== 'motion_layered') throw Error('Wrong family')
    for (const slot of ['body', 'eyes', 'hair', 'outfit', 'accessory'] as const) {
      expect(saved.config.item_ids[slot]).toBe(slot === item.slot ? item.item_id : before[slot])
    }
    expect((await f.persistence.read(f.signal.signal)).config).toEqual(config)
    if (config.representation !== 'motion_layered') throw Error('Wrong representation')
  }
  for (const slot of ['outfit', 'accessory'] as const) {
    config = decodeReviewedMotionConfig({ ...config, item_ids: { ...config.item_ids, [slot]: null } })
    expect((await f.persistence.save(revision++, config, f.signal.signal)).config).toEqual(config)
    if (config.representation !== 'motion_layered') throw Error('Wrong representation')
  }
  expect((await f.persistence.read(f.signal.signal)).config).toEqual(config)
})
it.each(['conflict', 'uncertain', 'denied', 'bad-revision'] as const)('does not retry or disguise a %s write', async mode => {
  const f = await fixture(); await f.persistence.read(f.signal.signal); f.mode(mode)
  const error = await f.persistence.save(1, preset(2), f.signal.signal).then(() => null, value => value)
  expect(error).toBeInstanceOf(TransportError)
  if (mode === 'conflict') expect(error).toMatchObject({ status: 409, server: { reason: 'VERSION_CONFLICT' }, uncertain: false })
  if (mode === 'denied') expect(error).toMatchObject({ status: 403, uncertain: false })
  if (mode === 'uncertain' || mode === 'bad-revision') expect(error).toMatchObject({ uncertain: true })
  expect(f.calls.filter(call => call.method === 'PUT')).toHaveLength(1)
})
it('rejects a different Auth owner even if its session scope values coincide', async () => {
  const f = await fixture(), foreign = await setup(); cleanup.push(foreign.dispose)
  expect(() => createReviewedMotionPersistence({ auth: foreign.auth, authority: f.authority })).toThrow()
})
it('honors the authenticated mutation capability without inventing local eligibility', async () => {
  const f = await setup(); cleanup.push(f.dispose)
  object(f.bundle().capabilities).mutations = { available: false, reason: 'FEATURE_DISABLED' }
  const authority = await f.create(); cleanup.push(authority.dispose)
  const persistence = createReviewedMotionPersistence({ auth: f.auth, authority }); cleanup.push(persistence.dispose)
  expect(authority.canSave).toBe(false)
  expect((await persistence.read(f.signal.signal)).config).toEqual(preset(0))
  await expect(persistence.save(1, preset(1), f.signal.signal)).rejects.toThrow()
  expect(f.calls.filter(c => c.method === 'PUT')).toHaveLength(0)
})
it.each(['foreign user', 'private profile', 'old revision', 'same revision different config', 'foreign family'])(
  'rejects invalid authoritative participant reads: %s', async kind => {
    const f = await fixture(); await f.persistence.read(f.signal.signal)
    if (kind === 'old revision') { await f.persistence.save(1, preset(1), f.signal.signal); f.setServer({ revision: 1, config: preset(0) }) }
    if (kind === 'same revision different config') f.setServer({ revision: 1, config: preset(1) })
    if (kind === 'foreign user') object(f.bundle().participant).user_id = '00000000-0000-4000-8000-000000000099'
    if (kind === 'private profile') object(object(f.bundle().participant).profile).email = 'private@example.invalid'
    if (kind === 'foreign family') f.handler(() => {
      const participant = object(structuredClone(f.bundle().participant))
      object(participant.profile).avatar = { revision: 1, config: { ...preset(0), manifest_id: 'mcr_selected_static_20261103' } }
      return Response.json(participant)
    })
    await expect(f.persistence.read(f.signal.signal)).rejects.toThrow()
  })
it('aborts a late response and prevents further private requests after authority invalidation', async () => {
  const f = await fixture(), response = deferred<Response>()
  f.handler(() => response.promise)
  const pending = f.persistence.read(f.signal.signal).then(() => false, () => true)
  await vi.waitFor(() => expect(f.calls).toHaveLength(2))
  f.authority.dispose(); response.resolve(Response.json(f.bundle().participant))
  expect(await pending).toBe(true)
  await expect(f.persistence.read(f.signal.signal)).rejects.toThrow()
  await expect(f.persistence.save(1, preset(1), f.signal.signal)).rejects.toThrow()
  expect(f.calls).toHaveLength(2)
})
it('does not mutate or silently convert the original static family during rejected production creation', async () => {
  const f = await setup(); cleanup.push(f.dispose)
  const legacy = { ...preset(0), contract_version: '1.1.0', manifest_id: 'mcr_selected_static_20261103', manifest_version: 1, representation: 'whole_character', preset_id: 'photo01_front_01' }
  const b = f.bundle(); object(b.avatar).config = structuredClone(legacy); object(object(b.participant).profile).avatar = b.avatar
  const before = structuredClone(b)
  await expect(createReviewedProductionAuthority({ auth: f.auth, scope: f.scope, pin: f.pin, signal: f.signal.signal })).rejects.toThrow()
  expect(b).toEqual(before); expect(f.calls.filter(c => c.method !== 'GET')).toHaveLength(0)
})
