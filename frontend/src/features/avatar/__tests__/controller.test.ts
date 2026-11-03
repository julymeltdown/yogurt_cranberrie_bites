// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createAvatarEditorController } from '../controller'
import { alternate, deferred, eventId, harness, original, participant, testPreview, userId } from './fixtures/harness'
import type { AvatarPreviewPort, PreviewLease } from '../contract'
const owned: { dispose(): void }[] = []
afterEach(() => owned.splice(0).forEach(item => item.dispose()))
async function setup() { const f = harness(); owned.push(f); f.controller.start(); await vi.waitFor(() => expect(f.controller.read().preview.phase).toBe('ready')); return f }
const writes = (f: ReturnType<typeof harness>) => f.requests.filter(r => r.method === 'PUT')
describe('FE13 draft and canonical revision saves', () => {
  it('loads verified current identity and does not write until explicit save', async () => {
    const f = await setup(); expect(f.controller.read().persisted).toEqual({ revision: 1, config: original })
    expect(f.controller.selectPreset('fixture_alternate')).toBe(true)
    expect(f.controller.read().draft).toEqual(alternate); expect(writes(f)).toHaveLength(0)
    expect(f.controller.cancel()).toBe(true); expect(f.controller.read().draft).toEqual(original)
    expect(f.server.config).toEqual(original)
    f.controller.selectItem('hair', 'hair_second'); expect(await f.controller.save()).toBe(true)
    expect(writes(f)).toHaveLength(1); expect(writes(f)[0]?.body).toEqual({ expected_revision: 1, config: alternate })
    expect(f.commits.at(-1)).toEqual({ revision: 2, config: alternate })
    expect(f.controller.read().persisted).toEqual(f.server)
    expect(await f.controller.save()).toBe(false)
  })
  it('rejects unknown, wrong-slot, historical source-sheet and unsupported selections', async () => {
    const f = await setup()
    for (const id of ['missing', 'photo_01', 'body_fixture']) expect(f.controller.selectItem('hair', id)).toBe(false)
    expect(f.controller.selectPreset('missing')).toBe(false)
    expect(f.controller.selectTone('missing')).toBe(false)
    expect(f.controller.selectPreset('fixture_unsupported')).toBe(false)
    expect(f.controller.selectTone('tone_unsupported')).toBe(false)
    expect(f.controller.read().draft).toEqual(original)
    expect(f.controller.selectItem('accessory', null)).toBe(true)
    expect(await f.controller.save()).toBe(true)
    expect(f.server.config.item_ids.accessory).toBeNull()
  })
  it('keeps failed drafts and does not expose raw server messages or replay denied writes', async () => {
    const f = await setup(); f.controller.selectPreset('fixture_alternate'); f.setMode('denied')
    expect(await f.controller.save()).toBe(false)
    expect(f.controller.read()).toMatchObject({ draft: alternate, notice: 'save-failed', needsReconciliation: false })
    expect(JSON.stringify(f.controller.read())).not.toContain('private server message'); expect(writes(f)).toHaveLength(1)
  })
  it('preserves draft across 409, reads latest explicitly, and rebases only by choice', async () => {
    const f = await setup(); f.controller.selectPreset('fixture_alternate'); f.setMode('conflict')
    expect(await f.controller.save()).toBe(false)
    expect(f.controller.read()).toMatchObject({ draft: alternate, notice: 'conflict', needsReconciliation: true })
    expect(await f.controller.save()).toBe(false); expect(writes(f)).toHaveLength(1)
    f.setServer({ revision: 7, config: original }); f.setMode('ok'); await f.controller.reload()
    expect(f.controller.read()).toMatchObject({ persisted: { revision: 1 }, draft: alternate, candidate: { revision: 7 }, needsReconciliation: true })
    expect(await f.controller.save()).toBe(false)
    expect(f.controller.resolveConflict('keep-draft')).toBe(true); expect(writes(f)).toHaveLength(1)
    expect(await f.controller.save()).toBe(true); expect(writes(f)[1]?.body).toMatchObject({ expected_revision: 7 })
  })
  it.each(['uncertain', 'invalid'] as const)('gates %s response without automatically repeating a write', async mode => {
    const f = await setup(); f.controller.selectPreset('fixture_alternate'); f.setMode(mode)
    expect(await f.controller.save()).toBe(false); expect(f.controller.read().needsReconciliation).toBe(true)
    expect(f.controller.read().notice).toBe('uncertain'); expect(await f.controller.save()).toBe(false)
    expect(writes(f)).toHaveLength(1); f.setMode('ok'); await f.controller.reload()
    expect(f.controller.resolveConflict('use-server')).toBe(true)
    expect(f.controller.read().draft).toEqual(alternate); expect(f.controller.read().persisted?.revision).toBe(2)
    expect(await f.controller.save()).toBe(false); expect(writes(f)).toHaveLength(1)
  })
  it('does not discard reconciliation requirement when cancelling an uncertain draft', async () => {
    const f = await setup(); f.controller.selectPreset('fixture_alternate'); f.setMode('uncertain'); await f.controller.save()
    f.controller.cancel(); expect(f.controller.read().needsReconciliation).toBe(true)
    f.controller.selectPreset('fixture_alternate'); expect(await f.controller.save()).toBe(false)
    f.setMode('read-failed'); await f.controller.reload(); expect(f.controller.read().draft).toEqual(alternate)
    expect(f.controller.read().needsReconciliation).toBe(true)
  })
  it('rejects stale reads and wrong-identity participants', async () => {
    const f = await setup(); f.controller.selectPreset('fixture_alternate'); await f.controller.save()
    f.setMode('stale-read'); await f.controller.reload(); expect(f.controller.read().notice).toBe('load-failed')
    expect(f.controller.read().persisted?.revision).toBe(2)
    f.setMode('other-user'); await f.controller.reload(); expect(f.controller.read().candidate).toBeNull()
    expect(f.controller.read().notice).toBe('load-failed')
  })
  it('keeps a save single-flight and disables cancel/selection until its outcome', async () => {
    const f = await setup(); f.setMode('slow'); f.controller.selectPreset('fixture_alternate'); const result = f.controller.save()
    await vi.waitFor(() => expect(writes(f)).toHaveLength(1)); expect(await f.controller.save()).toBe(false)
    expect(f.controller.cancel()).toBe(false); expect(f.controller.selectPreset('fixture_original')).toBe(false)
    f.pendingSave.resolve(Response.json({ revision: 2, config: alternate })); expect(await result).toBe(true)
  })
  it('drops old identity responses and scrubs preview before new subscribers observe state', async () => {
    const f = await setup(); f.setMode('slow'); f.controller.selectPreset('fixture_alternate'); const result = f.controller.save()
    await vi.waitFor(() => expect(writes(f)).toHaveLength(1)); const old = f.previews[0]!, canvases = [...old.leases]
    f.session.replace(f.session.read().generation, { userId: '00000000-0000-4000-8000-000000000102', eventId, accessToken: 'other-token' })
    expect(old.disposed).toBe(true); expect(canvases.every(c => c.width === 0)).toBe(true)
    expect(f.controller.read().draft).toBeNull()
    f.pendingSave.resolve(Response.json({ revision: 2, config: alternate })); expect(await result).toBe(false)
    expect(f.commits).toHaveLength(0)
  })
  it.each(['block', 'suspended', 'event-closed'] as const)('tears down on privacy policy %s', async reason => {
    const f = await setup(), old = f.previews[0]!
    f.privacy.changePolicy(f.privacy.capture(), reason)
    expect(old.disposed).toBe(true); expect(old.leases.size).toBe(0)
    expect(f.controller.read().draft).toBeNull(); expect(await f.controller.save()).toBe(false)
  })
  it('releases selection-churn results even when preview ignores abort', async () => {
    const f = harness(); owned.push(f); const port = testPreview(); let resolve!: (lease: PreviewLease) => void
    const pending = new Promise<PreviewLease>(yes => { resolve = yes }), leases: PreviewLease[] = []
    const render = port.port.render
    port.port.render = async (config, signal) => { const lease = await render(config, signal); leases.push(lease); return config.item_ids.hair === 'hair_second' ? pending : lease }
    const controller = createAvatarEditorController({ auth: { privacy: f.privacy, request: async request => {
      return { data: request.decode(participant, { scope: f.session.read(), status: 200, path: request.path }), scope: f.session.read() }
    } }, createPreview: async () => port.port }); owned.push(controller); controller.start()
    await vi.waitFor(() => expect(controller.read().preview.phase).toBe('ready'))
    controller.selectPreset('fixture_alternate'); await vi.waitFor(() => expect(leases).toHaveLength(2))
    controller.cancel(); await vi.waitFor(() => expect(controller.read().preview.phase).toBe('ready'))
    const current = controller.read().preview.canvas; resolve(leases[1]!)
    await Promise.resolve(); await Promise.resolve(); expect(controller.read().preview.canvas).toBe(current); expect(leases[1]!.canvas.width).toBe(0)
  })
  it('disposes late initialization and allows strict-effect stop/start without carrying drafts', async () => {
    const f = harness(); owned.push(f); const late = deferred<AvatarPreviewPort>(), port = testPreview()
    const c = createAvatarEditorController({ auth: { privacy: f.privacy, request: async () => { throw new Error('No read after stop') } }, createPreview: () => late.promise }); owned.push(c)
    c.start(); c.stop(); late.resolve(port.port); await Promise.resolve(); await Promise.resolve()
    expect(port.disposed).toBe(true); expect(c.read().phase).toBe('idle')
    f.controller.start(); await vi.waitFor(() => expect(f.controller.read().draft).toEqual(original))
    f.controller.selectPreset('fixture_alternate'); f.controller.stop(); expect(f.controller.read().draft).toBeNull()
    f.controller.start(); await vi.waitFor(() => expect(f.controller.read().draft).toEqual(original)); expect(userId).toBe(f.privacy.capture().userId)
  })
})
