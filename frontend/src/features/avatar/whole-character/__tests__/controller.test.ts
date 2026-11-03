// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { configs, harness, initial } from './fixtures'
import { decodeWholeConfig, decodeWholeParticipant } from '../model'
import participant from '../../../../../../contracts/fixtures/core/private-participant.json'
const owned: ReturnType<typeof harness>[] = []
afterEach(() => { owned.splice(0).forEach(h => h.dispose()); vi.restoreAllMocks() })
async function setup() { const h = harness(); owned.push(h); h.controller.start(); await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready')); return h }
const puts = (h: ReturnType<typeof harness>) => h.requests.filter(r => r.method === 'PUT')
describe('whole-character scoped editor', () => {
  it('admits only the exact whole variant; rejects layered/extra keys and noncanonical IDs', () => {
    expect(decodeWholeConfig(configs[0])).toEqual(initial.config)
    for (const patch of [{ skin_tone_id: 'fake' }, { item_ids: {} }, { preset_id: 'photo01-front-01' }, { manifest_version: 2 }, { contract_version: '1.0.0' }]) expect(() => decodeWholeConfig({ ...initial.config, ...patch })).toThrow()
  })
  it('validates the complete self projection and keeps participant and avatar revisions distinct', () => {
    const h = harness(); owned.push(h)
    const value = { ...participant, revision: 42, profile: { ...participant.profile, avatar: { revision: 7, config: configs[0] } } }
    expect(decodeWholeParticipant(value, h.scope)).toEqual({ revision: 7, config: configs[0] })
    for (const invalid of [{ ...value, avatar: initial }, { ...value, preferences: {} }, { ...value, profile: { ...value.profile, secret: 'invalid' } }, { ...value, profile: { ...value.profile, user_id: '00000000-0000-4000-8000-000000000099' } }]) expect(() => decodeWholeParticipant(invalid, h.scope)).toThrow()
  })
  it('loads canonical self without an invented default; local selection/random/reset make no PUT', async () => {
    const h = await setup(); expect(h.controller.read().persisted).toEqual(initial); expect(h.controller.read().choices).toHaveLength(6)
    expect(h.controller.select('unknown')).toBe(false); expect(h.controller.read().draft).toEqual(initial.config)
    expect(h.controller.select(configs[1]!.preset_id)).toBe(true); expect(h.controller.read().draft).toEqual(configs[1])
    vi.spyOn(Math, 'random').mockReturnValue(0); expect(h.controller.random()).toBe(true); expect(h.controller.read().draft).not.toEqual(configs[1])
    expect(h.controller.reset()).toBe(true); expect(h.controller.read().draft).toEqual(initial.config); expect(puts(h)).toHaveLength(0); expect(h.commits).toHaveLength(0)
  })
  it('saves one exact revision-bound response through canonical HTTP and only then notifies root', async () => {
    const h = await setup(); h.controller.select(configs[1]!.preset_id)
    expect(await h.controller.save()).toBe(true); expect(puts(h)).toHaveLength(1)
    expect(puts(h)[0]!.body).toEqual({ expected_revision: 1, config: configs[1] })
    expect(puts(h)[0]!.headers.get('X-MCR-Intent')).toBe('browser'); expect(puts(h)[0]!.headers.get('Authorization')).toBe('Bearer whole-fixture-token')
    expect(puts(h)[0]!.headers.get('Idempotency-Key')).toBeNull(); expect(h.commits).toEqual([{ revision: 2, config: configs[1] }])
    expect(await h.controller.save()).toBe(false); expect(puts(h)).toHaveLength(1)
  })
  it.each(['conflict', 'uncertain', 'invalid'] as const)('preserves %s draft and requires explicit latest-state decision', async mode => {
    const h = await setup(); h.controller.select(configs[1]!.preset_id); h.setMode(mode)
    expect(await h.controller.save()).toBe(false); expect(h.controller.read().draft).toEqual(configs[1]); expect(h.controller.read().needsReconciliation).toBe(true); expect(h.commits).toHaveLength(0)
    const choicesBefore = h.stats().choiceCalls; h.controller.retry(); await Promise.resolve()
    expect(h.stats().choiceCalls).toBe(choicesBefore); expect(h.controller.read().draft).toEqual(configs[1]); expect(h.controller.read().needsReconciliation).toBe(true)
    expect(h.controller.reset()).toBe(true); expect(h.controller.read().needsReconciliation).toBe(true); expect(await h.controller.save()).toBe(false)
    h.setMode('ok'); h.setServer({ revision: 4, config: configs[2]! }); expect(await h.controller.reload()).toBe(true)
    const candidate = h.controller.read().candidate; expect(h.controller.reset()).toBe(false); expect(h.controller.read().candidate).toBe(candidate)
    expect(h.controller.resolveConflict('keep-draft')).toBe(true); expect(puts(h)).toHaveLength(1); expect(h.controller.read().draft).toEqual(initial.config)
    expect(await h.controller.save()).toBe(true); expect(puts(h)[1]!.body).toEqual({ expected_revision: 4, config: initial.config })
  })
  it('keeps denied draft without automatic replay and refuses stale/cross-user reads', async () => {
    const h = await setup(); h.controller.select(configs[1]!.preset_id); h.setMode('denied'); expect(await h.controller.save()).toBe(false)
    expect(h.controller.read().notice).toBe('save-failed'); expect(h.controller.read().needsReconciliation).toBe(false); expect(puts(h)).toHaveLength(1)
    h.setMode('ok'); h.setServer({ revision: 1, config: configs[2]! }); expect(await h.controller.reload()).toBe(false); expect(h.controller.read().candidate).toBeNull()
    h.setMode('other-user'); expect(await h.controller.reload()).toBe(false); expect(h.controller.read().draft).toEqual(configs[1])
  })
  it('prevents duplicate/pending actions and late commits after logout without disposing borrowed runtime', async () => {
    const h = await setup(); h.controller.select(configs[1]!.preset_id); h.setMode('slow'); const save = h.controller.save()
    await vi.waitFor(() => expect(puts(h)).toHaveLength(1)); expect(await h.controller.save()).toBe(false); expect(h.controller.reset()).toBe(false); expect(h.controller.random()).toBe(false)
    h.session.replace(h.session.read().generation, null)
    expect(h.controller.read().draft).toBeNull(); expect(h.controller.read().choices).toHaveLength(0)
    h.pending.resolve(Response.json({ revision: 2, config: configs[1] })); expect(await save).toBe(false)
    expect(h.commits).toHaveLength(0); expect(h.stats().runtimeDisposed).toBe(0); h.controller.dispose(); expect(h.stats().runtimeDisposed).toBe(0)
  })
  it('keeps a warm unsaved draft editable when the verified baseline is unchanged; privacy never rebinds', async () => {
    const h = await setup(); h.controller.select(configs[3]!.preset_id); h.controller.stop(); expect(h.controller.read().draft).toBeNull(); expect(h.stats().runtimeDisposed).toBe(0)
    h.controller.start(); await vi.waitFor(() => expect(h.controller.read().phase).toBe('ready')); expect(h.controller.read().draft).toEqual(configs[3]); expect(puts(h)).toHaveLength(0)
    expect(h.controller.read().candidate).toBeNull(); expect(h.controller.read().needsReconciliation).toBe(false); expect(h.controller.select(configs[4]!.preset_id)).toBe(true)
    h.privacy.changePolicy(h.scope, 'consent-revoked'); expect(h.controller.read().draft).toBeNull(); h.controller.start(); expect(h.controller.read().phase).toBe('unavailable'); expect(h.stats().runtimeDisposed).toBe(0)
  })
  it('requires a decision on changed warm baseline or unresolved conflict even if the baseline matches', async () => {
    const h = await setup(); h.controller.select(configs[1]!.preset_id); h.controller.stop(); h.setServer({ revision: 2, config: configs[2]! })
    h.controller.start(); await vi.waitFor(() => expect(h.controller.read().candidate?.revision).toBe(2)); expect(h.controller.read().draft).toEqual(configs[1]); expect(h.controller.read().needsReconciliation).toBe(true)
    h.controller.resolveConflict('keep-draft'); h.setMode('conflict'); await h.controller.save(); h.controller.stop()
    h.setMode('ok'); h.controller.start(); await vi.waitFor(() => expect(h.controller.read().candidate?.revision).toBe(2))
    expect(h.controller.read().needsReconciliation).toBe(true); expect(await h.controller.save()).toBe(false); expect(puts(h)).toHaveLength(1)
  })
  it('blocks a retained draft after failed fresh verification and never turns retry into a reset', async () => {
    const h = await setup(); h.controller.select(configs[1]!.preset_id); h.controller.stop(); h.setMode('read-failed'); h.controller.start()
    await vi.waitFor(() => expect(h.controller.read().notice).toBe('load-failed'))
    expect(h.controller.read().draft).toEqual(configs[1]); expect(h.controller.read().needsReconciliation).toBe(true)
    h.controller.retry(); expect(await h.controller.save()).toBe(false); expect(puts(h)).toHaveLength(0)
    h.setMode('ok'); expect(await h.controller.reload()).toBe(true); expect(h.controller.read().candidate).toEqual(initial)
    expect(h.controller.resolveConflict('keep-draft')).toBe(true); expect(h.controller.read().draft).toEqual(configs[1])
  })
  it('rejects regressions against an observed candidate across repeated reload and stop/start', async () => {
    const h = await setup(); h.controller.select(configs[1]!.preset_id)
    h.setServer({ revision: 4, config: configs[2]! }); expect(await h.controller.reload()).toBe(true)
    h.setServer({ revision: 2, config: configs[3]! }); expect(await h.controller.reload()).toBe(false); expect(h.commits).toHaveLength(0)
    h.setServer({ revision: 4, config: configs[4]! }); expect(await h.controller.reload()).toBe(false)
    h.controller.stop(); h.controller.start(); await vi.waitFor(() => expect(h.controller.read().notice).toBe('load-failed'))
    expect(h.controller.read().draft).toEqual(configs[1]); expect(h.controller.read().needsReconciliation).toBe(true)
    h.setServer({ revision: 4, config: configs[2]! }); expect(await h.controller.reload()).toBe(true)
    expect(h.controller.resolveConflict('use-server')).toBe(true); expect(h.commits).toEqual([{ revision: 4, config: configs[2] }])
  })
})
