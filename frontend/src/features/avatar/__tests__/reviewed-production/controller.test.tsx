// @vitest-environment jsdom
import { readFile } from 'node:fs/promises'
import { URL as FileURL } from 'node:url'
import { afterEach, beforeAll, expect, it, vi } from 'vitest'
import { createMotionEditorController } from '../../motion-adoption/controller'
import { createMotionRuntime } from '../../../../entities/avatar/runtime/motion-adoption/runtime'
import { createReviewedMotionFixtureAuthority, decodeReviewedMotionManifest } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-manifest'
import { reviewedMotionSelection } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-config'
import { validateMotionArt, type MotionArt, type MotionSelection } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { TransportError } from '../../../../shared/api/errors'
import { manifest, pin, preset } from './fixtures'

let art: MotionArt
beforeAll(async () => { art = await validateMotionArt(new Uint8Array(await readFile(new FileURL('../../../../../public/world/avatars/derived-v1/motion-candidates-v1/combined-release-candidate.json', import.meta.url)))) })
const cleanup: (() => void)[] = []
afterEach(() => { cleanup.splice(0).reverse().forEach(fn => fn()); vi.restoreAllMocks() })
const settle = async () => { for (let i = 0; i < 50; i++) await Promise.resolve() }
async function fixture() {
  const stage = await decodeReviewedMotionManifest(manifest, pin), authority = createReviewedMotionFixtureAuthority(stage)
  let alive = true, mode: 'ok' | 'conflict' | 'uncertain' = 'ok'
  const cleanups = new Set<() => void>(), privacy = { isCurrent: () => alive, registerCleanup(fn: () => void) { cleanups.add(fn); return () => { cleanups.delete(fn) } } }
  const runtime = createMotionRuntime({ authority, scope: { userId: 'self', eventId: 'event', generation: 1, policyGeneration: 1, gate: 'open' },
    privacy, origin: 'https://mcr.test', load: async () => ({ art, image: new Image(), dispose() {} }) })
  let server = { revision: 1, config: preset(0) }
  const read = vi.fn(async () => structuredClone(server)), save = vi.fn(async (revision: number, config: typeof server.config) => {
    if (mode === 'conflict') throw new TransportError('http', { status: 409, server: {
      envelope: { code: 'CONFLICT', message: 'Changed', details: { reason: 'VERSION_CONFLICT' } }, core: { reason: 'VERSION_CONFLICT' }, reason: 'VERSION_CONFLICT' } })
    expect(revision).toBe(server.revision); server = { revision: revision + 1, config: authority.decodeConfig(config) }
    if (mode === 'uncertain') throw Error('Response lost')
    return structuredClone(server)
  })
  const controller = createMotionEditorController({ runtime, privacy, persistence: { read, save } })
  cleanup.push(runtime.dispose, controller.dispose)
  controller.start(); await settle(); expect(controller.read().phase).toBe('ready')
  return { runtime, controller, read, save, server: () => server, mode: (next: typeof mode) => { mode = next },
    revoke() { alive = false; for (const fn of Array.from(cleanups)) fn() } }
}
it('offers exactly six presets and independent 8/6/4/6/4 source choices, with no automatic save', async () => {
  const f = await fixture(), catalog = f.controller.read().catalog!
  expect(catalog.presets).toHaveLength(6)
  expect(Object.fromEntries(catalog.layers.map(layer => [layer, catalog.items.filter(item => item.kind === layer).length])))
    .toEqual({ body: 8, hair: 6, eyes: 4, outfit: 6, accessory: 4 })
  for (const choice of catalog.presets) { expect(f.controller.selectPreset(choice.id)).toBe(true); expect(f.controller.read().draft).toEqual(choice.config) }
  let selection: MotionSelection = { mode: 'wardrobe', body: 'body-0', hair: 'hair-0', eyes: 'eyes-0', outfit: null, accessory: null }
  for (const item of catalog.items) {
    const before = selection; selection = { ...selection, [item.kind]: item.key }
    expect(await f.controller.choose(selection)).toBe(true)
    const actual = reviewedMotionSelection(f.controller.read().draft)
    expect(actual).toEqual(selection)
    if (actual.mode !== 'wardrobe') throw Error('Mode changed')
    for (const slot of catalog.layers) expect(actual[slot]).toBe(slot === item.kind ? item.key : before[slot])
  }
  expect(f.save).not.toHaveBeenCalled(); expect(f.controller.reset()).toBe(true); expect(f.controller.read().draft).toEqual(preset(0))
})
it.each([0, 0.49, 0.999])('Random changes a preset without changing family or saving, sample%s', async random => {
  const f = await fixture(); vi.spyOn(Math, 'random').mockReturnValue(random)
  expect(await f.controller.random()).toBe(true)
  const config = f.controller.read().draft!
  expect(config).not.toEqual(preset(0)); expect(config.manifest_id).toBe(pin.manifest_id)
  expect(f.controller.read().catalog!.presets.some(p => JSON.stringify(p.config) === JSON.stringify(config))).toBe(true)
  expect(f.save).not.toHaveBeenCalled()
})
it('Random uses admitted independent IDs/nulls and handles a repeated sample without retries', async () => {
  const f = await fixture(); vi.spyOn(Math, 'random').mockReturnValue(0)
  await f.controller.choose({ mode: 'wardrobe', body: 'body-0', hair: 'hair-0', eyes: 'eyes-0', outfit: 'outfit-0', accessory: 'accessory-0' })
  expect(await f.controller.random()).toBe(true)
  expect(reviewedMotionSelection(f.controller.read().draft)).toEqual({ mode: 'wardrobe', body: 'body-0', hair: 'hair-1', eyes: 'eyes-0', outfit: 'outfit-0', accessory: 'accessory-0' })
  vi.spyOn(Math, 'random').mockReturnValue(0.999)
  await f.controller.random()
  expect(reviewedMotionSelection(f.controller.read().draft)).toEqual({ mode: 'wardrobe', body: 'body-7', hair: 'hair-5', eyes: 'eyes-3', outfit: null, accessory: null })
  expect(f.save).not.toHaveBeenCalled()
})
it('saves explicit independent selections and retains the exact persisted outfit after reentry/reset', async () => {
  const f = await fixture()
  await f.controller.choose({ mode: 'wardrobe', body: 'body-7', hair: 'hair-5', eyes: 'eyes-3', outfit: null, accessory: null })
  const draft = f.controller.read().draft
  expect(await f.controller.save()).toBe(true); expect(f.save).toHaveBeenCalledOnce()
  f.controller.stop(); f.controller.start(); await settle()
  expect(f.read).toHaveBeenCalledTimes(2); expect(f.controller.read().draft).toEqual(draft)
  f.controller.selectPreset(f.controller.read().catalog!.presets[0]!.id)
  f.controller.reset(); expect(f.controller.read().draft).toEqual(draft)
})
it.each(['conflict', 'uncertain'] as const)('preserves randomized draft and blocks writes until %s reconciliation', async mode => {
  const f = await fixture(); await f.controller.random(); const draft = f.controller.read().draft
  f.mode(mode); expect(await f.controller.save()).toBe(false)
  expect(f.controller.read()).toMatchObject({ draft, needsReconciliation: true, notice: mode })
  expect(await f.controller.save()).toBe(false)
  f.mode('ok'); await f.controller.reload(); expect(f.controller.resolveConflict('keep-draft')).toBe(true)
  expect(f.controller.read().draft).toEqual(draft); expect(f.controller.read().needsReconciliation).toBe(false)
})
it('revokes category/draft/random/save state synchronously and cannot restart it', async () => {
  const f = await fixture(); await f.controller.random(); f.revoke()
  expect(f.controller.read()).toMatchObject({ phase: 'unavailable', draft: null, catalog: null, persisted: null })
  expect(await f.controller.random()).toBe(false); expect(await f.controller.save()).toBe(false)
  f.controller.start(); expect(f.controller.read().phase).toBe('unavailable'); expect(f.save).not.toHaveBeenCalled()
})
