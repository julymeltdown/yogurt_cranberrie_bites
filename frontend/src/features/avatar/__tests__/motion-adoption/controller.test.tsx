// @vitest-environment jsdom
import { readFile } from 'node:fs/promises'
import { URL as FileURL } from 'node:url'
import { afterEach, beforeAll, expect, it, vi } from 'vitest'
import { validateMotionArt, type MotionArt } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { createMotionRuntime } from '../../../../entities/avatar/runtime/motion-adoption/runtime'
import { testMotionAuthority, type TestMotionConfig } from '../../../../world/avatar/__tests__/motion-adoption/test-authority'
import { createMotionEditorController } from '../../motion-adoption/controller'
import { TransportError } from '../../../../shared/api/errors'

let art: MotionArt
beforeAll(async () => { art = await validateMotionArt(new Uint8Array(await readFile(new FileURL('../../../../../public/world/avatars/derived-v1/motion-candidates-v1/combined-release-candidate.json', import.meta.url)))) })
const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).reverse().forEach(fn => fn()); vi.restoreAllMocks() })
const settle = async () => { for (let i = 0; i < 40; i++) await Promise.resolve() }
function fixture() {
  let alive = true, mode: 'ok' | 'conflict' | 'uncertain' | 'denied' | 'malformed' = 'ok'
  const cleanups = new Set<() => void>(), privacy = { isCurrent: () => alive, registerCleanup(fn: () => void) { cleanups.add(fn); return () => { cleanups.delete(fn) } } }
  const authority = testMotionAuthority(art), presets = authority.presets(art)
  const runtime = createMotionRuntime({ authority, scope: { userId: 'self', eventId: 'event', generation: 1, policyGeneration: 1, gate: 'open' }, privacy, origin: 'https://mcr.test',
    load: async () => ({ art, image: new Image(), dispose() {} }) })
  let server = { revision: 1, config: presets[0]!.config }
  const reads = vi.fn(async () => structuredClone(server)), writes = vi.fn(async (revision: number, config: TestMotionConfig) => {
    if (mode === 'conflict') throw new TransportError('http', { status: 409, server: {
      envelope: { code: 'CONFLICT', message: 'Changed', details: { reason: 'VERSION_CONFLICT' } }, core: { reason: 'VERSION_CONFLICT' }, reason: 'VERSION_CONFLICT' } })
    if (mode === 'denied') throw new TransportError('http', { status: 403 })
    expect(revision).toBe(server.revision)
    server = { revision: revision + 1, config: authority.decodeConfig(config) }
    if (mode === 'uncertain') throw Error('Dropped response after commit')
    return mode === 'malformed' ? { ...server, revision: 1 } : structuredClone(server)
  })
  const committed = vi.fn(), controller = createMotionEditorController({ runtime, privacy, persistence: { read: reads, save: writes }, onCommitted: committed })
  stops.push(runtime.dispose, controller.dispose)
  return { controller, runtime, reads, writes, committed, presets, server: () => server, mode: (next: typeof mode) => { mode = next },
    external(index: number) { server = { revision: server.revision + 1, config: presets[index]!.config } },
    revoke() { alive = false; for (const fn of Array.from(cleanups)) fn() } }
}
it('keeps draft local until explicit save; cancel/reset preserves saved config and verified commit refreshes once', async () => {
  const f = fixture(); f.controller.start(); await settle(); expect(f.controller.read().phase).toBe('ready')
  f.controller.selectPreset(f.presets[3]!.id); expect(f.writes).not.toHaveBeenCalled(); expect(f.server().config).toEqual(f.presets[0]!.config)
  f.controller.reset(); expect(f.controller.read().draft).toEqual(f.presets[0]!.config)
  f.controller.selectPreset(f.presets[2]!.id); expect(await f.controller.save()).toBe(true)
  expect(f.writes).toHaveBeenCalledOnce(); expect(f.committed).toHaveBeenCalledOnce(); expect(f.controller.read().persisted).toEqual(f.server())
  expect(await f.controller.save()).toBe(false)
})
it('supports actual source categories without treating local keys as public inventory', async () => {
  const f = fixture(); f.controller.start(); await settle()
  expect(await f.controller.choose({ mode: 'wardrobe', body: 'body-7', eyes: 'eyes-3', hair: 'hair-5', outfit: 'outfit-5', accessory: 'accessory-3' })).toBe(true)
  expect(f.controller.read().draft?.selection).toMatchObject({ mode: 'wardrobe', body: 'body-7', accessory: 'accessory-3' })
  expect(f.writes).not.toHaveBeenCalled(); expect(await f.controller.choose({ mode: 'preset', preset: 'not-an-id' })).toBe(false)
})
it('preserves dirty drafts across panel stop/start without disposing the borrowed runtime', async () => {
  const f = fixture(); f.controller.start(); await settle(); f.controller.selectPreset(f.presets[4]!.id)
  f.controller.stop(); expect(f.runtime.isCurrent()).toBe(true); f.controller.start(); await settle()
  expect(f.controller.read().draft).toEqual(f.presets[4]!.config); expect(f.controller.read().needsReconciliation).toBe(false)
})
it.each(['conflict', 'uncertain', 'malformed'] as const)('requires authoritative reconciliation after %s without losing the draft', async mode => {
  const f = fixture(); f.controller.start(); await settle(); f.controller.selectPreset(f.presets[4]!.id); f.mode(mode)
  expect(await f.controller.save()).toBe(false); expect(f.controller.read().needsReconciliation).toBe(true)
  expect(f.controller.read().draft).toEqual(f.presets[4]!.config); expect(f.committed).not.toHaveBeenCalled()
  expect(await f.controller.save()).toBe(false); f.mode('ok'); await f.controller.reload()
  expect(f.controller.read().candidate).toEqual(f.server()); expect(f.controller.resolveConflict('keep-draft')).toBe(true)
  expect(f.controller.read().needsReconciliation).toBe(false)
})
it('retains a draft after a known denied save and supports using a newly read server outfit', async () => {
  const f = fixture(); f.controller.start(); await settle(); f.controller.selectPreset(f.presets[1]!.id); f.mode('denied')
  expect(await f.controller.save()).toBe(false); expect(f.controller.read()).toMatchObject({ notice: 'save-failed', needsReconciliation: false })
  f.external(3); await f.controller.reload(); expect(f.controller.resolveConflict('use-server')).toBe(true)
  expect(f.controller.read().draft).toEqual(f.presets[3]!.config)
})
it('withdraws private state synchronously and cannot restart after privacy revocation', async () => {
  const f = fixture(); f.controller.start(); await settle(); f.controller.selectPreset(f.presets[1]!.id); f.revoke()
  expect(f.controller.read()).toMatchObject({ phase: 'unavailable', draft: null, persisted: null, catalog: null })
  f.controller.start(); expect(await f.controller.save()).toBe(false); expect(f.writes).not.toHaveBeenCalled()
})
