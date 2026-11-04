// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createRoot, type Root } from 'react-dom/client'
import { act } from 'react'
import { createMuralClosureEntry, type MuralClosureEntry } from '../index'
import { MuralClosurePanel } from '../MuralClosurePanel'
import { fixture, ids, partialJob } from './fixture'
let f: ReturnType<typeof fixture>, entry: MuralClosureEntry, root: Root | undefined, host: HTMLDivElement | undefined
beforeEach(async () => { vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true); f = fixture(); entry = await f.open() })
afterEach(async () => { await act(async () => root?.unmount()); root = undefined; host?.remove(); host = undefined; f.dispose(); vi.unstubAllGlobals() })
const pause = () => ({ operation: 'controlConcertMural' as const, body: { expected_revision: entry.wall.read().mural!.revision, action: 'PAUSE' as const, reason: 'Reviewed synthetic operation' } })
async function panel(view: 'mural' | 'closure') {
  if (!host) { host = document.createElement('div'); document.body.append(host); root = createRoot(host) }
  await act(async () => { root!.render(<MuralClosurePanel entry={entry} view={view} />) })
}
describe('FE39 current Root-delegated composition', () => {
  it('lazily starts once and uses the canonical authenticated request policy without new owners', async () => {
    expect(f.server.requests).toHaveLength(2)
    await entry.start('closure'); expect(f.server.requests).toHaveLength(2)
    await entry.start('mural'); await entry.start('mural')
    const reads = f.server.requests.filter(r => r.url.endsWith('/admin/mural'))
    expect(reads).toHaveLength(1); expect(reads[0]!.headers.get('Authorization')).toMatch(/^Bearer /)
    expect(reads[0]!.cache).toBe('no-store'); expect(entry.wall.read().phase).toBe('ready')
    expect(f.posts()).toHaveLength(0)
  })
  it('retains an ambiguous wall command across panel unmount/back and resolves it using reads only', async () => {
    await panel('mural'); f.server.mode = 'lost'
    await act(async () => { entry.wall.prepare(pause()); await entry.wall.confirm() })
    const command = entry.wall.read().command
    expect(command).not.toBeNull(); await panel('closure'); await panel('mural')
    expect(entry.wall.read().command).toBe(command); expect(f.posts()).toHaveLength(1)
    await act(async () => { await entry.wall.check() })
    expect(entry.wall.read()).toMatchObject({ command: null, mural: { revision: 11, state: 'PAUSED' } })
    expect(f.posts()).toHaveLength(1)
  })
  it('restores the exact named snapshot into a new revision through the composed canonical transport', async () => {
    f.server.wall = { ...f.server.wall, state: 'PAUSED' }; await panel('mural')
    await act(async () => { entry.wall.prepare({ operation: 'restoreConcertMural', body: { expected_revision: 10, snapshot_id: ids.restore, reason: 'Restore reviewed snapshot' } }) })
    expect(host!.querySelector('[aria-label="Confirm wall action"]')!.textContent).toContain(ids.restore)
    await act(async () => { await entry.wall.confirm() })
    expect(await f.posts()[0]!.json()).toEqual({ expected_revision: 10, snapshot_id: ids.restore, reason: 'Restore reviewed snapshot' })
    expect(entry.wall.read().mural).toMatchObject({ revision: 11, snapshot_id: ids.restore, state: 'PAUSED' })
  })
  it('exposes partial progress and preserves an unresolved resume across panel navigation without automatic replay', async () => {
    f.update({ ...f.current(), phase: 'READ_ONLY' }); await panel('closure')
    await act(async () => { await entry.closure.refresh(ids.job) })
    expect(host!.textContent).toContain('PARTIAL_FAILURE'); expect(host!.querySelectorAll('[aria-label="Closure steps"] li')).toHaveLength(7)
    f.server.mode = 'lost'
    await act(async () => { expect(entry.closure.prepareResume('Retry reviewed export')).toBe(true); await entry.closure.confirm() })
    const command = entry.closure.read().command; await panel('mural'); await panel('closure')
    expect(entry.closure.read().command).toBe(command); expect(f.server.admissions).toBe(1)
    await act(async () => { await entry.closure.refresh() }); expect(entry.closure.read().command).toBe(command)
    await act(async () => { await entry.closure.check() })
    expect(entry.closure.read()).toMatchObject({ command: null, job: { state: 'RUNNING', revision: 4 } })
    expect(entry.closure.read().job!.steps.slice(0, 5).every(s => s.state === 'SUCCEEDED')).toBe(true)
    expect(f.posts()).toHaveLength(1)
  })
  it.each(['actor', 'event', 'policy'] as const)('%s replacement disposes both controllers and rejects old full scope', async cause => {
    await entry.start('mural'); await entry.start('closure'); await entry.closure.refresh(ids.job)
    if (cause === 'policy') f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block')
    else f.auth.session.replace(f.auth.session.read().generation, { userId: cause === 'actor' ? ids.restore : ids.actor, eventId: cause === 'event' ? ids.restore : ids.event, accessToken: 'synthetic_replacement' })
    expect(entry.isDisposed()).toBe(true); expect(entry.wall.read().mural).toBeNull(); expect(entry.closure.read().job).toBeNull()
    expect(() => createMuralClosureEntry({ auth: f.auth, scope: f.current().scope, context: f.context })).toThrow('Admin scope unavailable')
    const count = f.server.requests.length; await entry.start('mural'); expect(f.server.requests).toHaveLength(count)
    expect(f.listeners()).toBe(0)
  })
  it('fails closed with absent or mismatched authority and hides previous private progress on revocation', async () => {
    await panel('mural'); const count = f.server.requests.length
    await act(async () => { f.update(null) }); expect(host!.textContent).toContain('Event operations unavailable')
    await entry.wall.load(); expect(f.server.requests).toHaveLength(count)
    f.update({ scope: { ...f.auth.privacy.capture(), policyGeneration: 99 }, role: 'OPERATOR', roleRevision: 2, eventRevision: 8, phase: 'LIVE' })
    expect(entry.isCurrent()).toBe(false); await entry.wall.load(); expect(f.server.requests).toHaveLength(count)
  })
  it('current server denial remains failure despite a supplied operator context', async () => {
    f.server.operator = false; await entry.start('mural')
    expect(entry.isCurrent()).toBe(true); expect(entry.wall.read()).toMatchObject({ phase: 'error', mural: null })
    expect(f.posts()).toHaveLength(0)
  })
  it('aborts held reads and prevents late data from reappearing after ownership disposal', async () => {
    let resolve!: (value: Response) => void, held: Request | undefined
    f.server.intercept = request => request.url.endsWith('/admin/mural') ? new Promise(done => { held = request; resolve = done }) : null
    const pending = entry.start('mural'); await vi.waitFor(() => expect(held).toBeDefined()); entry.dispose()
    expect(held!.signal.aborted).toBe(true); resolve(Response.json(f.server.wall)); await pending
    expect(entry.wall.read()).toMatchObject({ phase: 'disposed', mural: null }); expect(f.listeners()).toBe(0)
  })
  it('uses current job revision for a later explicit boundary without rewinding completed progress', async () => {
    f.update({ ...f.current(), phase: 'READ_ONLY' }); await entry.start('closure'); await entry.closure.refresh(ids.job)
    expect(entry.closure.prepareResume('First boundary')).toBe(true); await entry.closure.confirm()
    f.server.job = partialJob(5); await entry.closure.refresh()
    expect(entry.closure.prepareResume('Later boundary')).toBe(true); await entry.closure.confirm()
    expect(f.posts()).toHaveLength(2)
    expect(f.posts()[0]!.headers.get('Idempotency-Key')).not.toBe(f.posts()[1]!.headers.get('Idempotency-Key'))
    expect(await f.posts()[1]!.json()).toMatchObject({ expected_revision: 5, confirmation: 'RESUME_CLOSURE', job_id: ids.job })
  })
})
