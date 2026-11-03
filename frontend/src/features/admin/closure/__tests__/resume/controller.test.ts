import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fixture, ids, job } from './fixture'
let f: ReturnType<typeof fixture>
beforeEach(async () => { vi.stubGlobal('location', new URL('https://mcr.test')); f = fixture(); await f.open() })
afterEach(() => { f.cleanup(); vi.useRealTimers(); vi.unstubAllGlobals() })
const prepare = () => f.controller.prepareResume('Retry failed export after recovery')
describe('explicit failed boundary resume', () => {
  it('review/cancel do not mutate; confirm uses the exact job revision and one fresh immutable key', async () => {
    expect(prepare()).toBe(true); expect(f.controller.read().draft).toEqual({ confirmation: 'RESUME_CLOSURE', job_id: ids.job, expected_revision: 8, reason: 'Retry failed export after recovery' })
    f.controller.cancel(); expect(f.posts()).toHaveLength(0); expect(prepare()).toBe(true)
    const previous = f.controller.read().job!.steps.slice(0, 5); await f.controller.confirm(); await f.controller.confirm()
    expect(f.posts()).toHaveLength(1); expect(await f.posts()[0]!.clone().json()).toEqual({ confirmation: 'RESUME_CLOSURE', job_id: ids.job, expected_revision: 8, reason: 'Retry failed export after recovery' })
    expect(f.posts()[0]!.headers.get('X-MCR-Intent')).toBe('browser'); expect(f.posts()[0]!.cache).toBe('no-store')
    expect(f.controller.read()).toMatchObject({ command: null, commandPhase: 'confirmed', job: { job_id: ids.job, state: 'RUNNING', revision: 9 } })
    expect(f.controller.read().job!.steps.slice(0, 5)).toEqual(previous); expect(f.server.admissions).toBe(1)
  })
  it('START and a later explicit RESUME use different revision subjects and different keys', async () => {
    f.cleanup(); f = fixture({ existing: false }); await f.open()
    expect(f.controller.prepare('Close after show')).toBe(true); await f.controller.confirm(); const startKey = f.posts()[0]!.headers.get('Idempotency-Key')
    expect(await f.posts()[0]!.clone().json()).toEqual({ confirmation: 'CLOSE_EVENT', expected_revision: 23, reason: 'Close after show' })
    f.failAgain(); f.syncContext(); await f.controller.refresh(); expect(prepare()).toBe(true); await f.controller.confirm()
    expect(await f.posts()[1]!.clone().json()).toMatchObject({ confirmation: 'RESUME_CLOSURE', expected_revision: 2, job_id: ids.job })
    expect(f.posts()[1]!.headers.get('Idempotency-Key')).not.toBe(startKey); expect(f.server.eventRevision).toBe(24)
  })
  it('success means admission, not completion; refreshing cannot resume a failed job or duplicate completed owner effects', async () => {
    const original = structuredClone(f.server.job)
    await f.controller.refresh(); await f.controller.refresh(); expect(f.server.job).toEqual(original); expect(f.server.admissions).toBe(0)
    prepare(); await f.controller.confirm(); expect(f.controller.read().job!.state).toBe('RUNNING')
    expect(Array.from(f.server.effects.values())).toEqual([1,1,1,1,1,0,0])
    f.complete(); f.syncContext(); await f.controller.refresh(); expect(f.controller.read().job!.state).toBe('COMPLETED')
    expect(Array.from(f.server.effects.values())).toEqual([1,1,1,1,1,1,1]); expect(f.server.admissions).toBe(1); expect(f.posts()).toHaveLength(1)
  })
  it.each(['DRAFT','OPEN','LIVE','ARCHIVED'] as const)('rejects resume in event phase %s', phase => {
    f.update({ ...f.context(), phase }); expect(prepare()).toBe(false); expect(f.posts()).toHaveLength(0)
  })
  it.each(['PENDING','RUNNING','COMPLETED'] as const)('rejects resume for job state %s', async state => {
    f.server.job = job(state, 9); await f.controller.refresh(); expect(prepare()).toBe(false); expect(f.posts()).toHaveLength(0)
  })
  it.each(['', '   ', 'x'.repeat(501)])('requires a bounded nonblank reason', reason => { expect(f.controller.prepareResume(reason)).toBe(false); expect(f.posts()).toHaveLength(0) })
  it('server CAS conflict withdraws the preview and requires a fresh read and new explicit confirmation', async () => {
    prepare(); f.failAgain(); await f.controller.confirm()
    expect(f.controller.read()).toMatchObject({ command: null, commandPhase: 'rejected', phase: 'error', job: null }); expect(prepare()).toBe(false)
    await f.controller.refresh(); expect(prepare()).toBe(true); await f.controller.confirm()
    expect(f.posts()).toHaveLength(2); expect(f.posts()[0]!.headers.get('Idempotency-Key')).not.toBe(f.posts()[1]!.headers.get('Idempotency-Key'))
    expect(f.controller.read().job!.revision).toBe(10)
  })
  it('cannot reuse a previously issued key for another failed boundary', async () => {
    f.cleanup(); f = fixture({ newKey: () => '00000000-0000-4000-8000-000000000800' }); await f.open()
    prepare(); await f.controller.confirm(); f.failAgain(); await f.controller.refresh(); prepare(); await f.controller.confirm()
    expect(f.posts()).toHaveLength(1); expect(f.controller.read().message).toContain('fresh command identity'); expect(f.controller.read().command).toBeNull()
  })
  it('lost admission keeps the original identity across current job reads and cannot become another resume', async () => {
    f.server.mode = 'lost'; prepare(); await f.controller.confirm(); const original = f.command()
    expect(f.controller.read().commandPhase).toBe('unknown'); expect(f.server.admissions).toBe(1)
    f.failAgain(); await f.controller.refresh(); expect(f.controller.read().job!.state).toBe('PARTIAL_FAILURE')
    expect(prepare()).toBe(false); await f.controller.confirm(); expect(f.command()).toBe(original)
    await f.controller.check(); expect(f.controller.read()).toMatchObject({ command: null, commandPhase: 'confirmed', job: { state: 'PARTIAL_FAILURE', revision: 10 } })
    expect(f.posts()).toHaveLength(1); expect(f.server.admissions).toBe(1)
  })
  it('202 is pending admission; explicit status/GET never admit it, and later success recovers only the same job', async () => {
    f.server.mode = 'pending'; prepare(); await f.controller.confirm(); const original = f.command()
    await f.controller.check(); await f.controller.refresh(); expect(f.command()).toBe(original); expect(f.server.admissions).toBe(0); expect(prepare()).toBe(false)
    f.settle(); await f.controller.check(); expect(f.controller.read()).toMatchObject({ command: null, commandPhase: 'confirmed', job: { revision: 9, state: 'RUNNING' } })
    expect(f.posts()).toHaveLength(1); expect(f.server.admissions).toBe(1)
  })
  it('503 plus inaccessible status does not prove nonexecution or authorize a new key', async () => {
    f.server.mode = 'unavailable'; prepare(); await f.controller.confirm(); const key = f.command().key
    await f.controller.check(); await f.controller.refresh(); expect(prepare()).toBe(false); expect(f.command().key).toBe(key); expect(f.posts()).toHaveLength(1)
  })
  it.each(['role','owner','event','policy','event-revision'])('%s change invalidates preview and late publication', async change => {
    prepare()
    if (change === 'role') f.update(null)
    if (change === 'owner') f.login(ids.other)
    if (change === 'event') f.login(ids.actor, ids.other)
    if (change === 'policy') f.privacy.changePolicy(f.privacy.capture(), 'suspended')
    if (change === 'event-revision') f.update({ ...f.context(), eventRevision: 24 })
    expect(f.controller.read()).toMatchObject({ draft: null, job: null, phase: 'unavailable' }); await f.controller.confirm(); expect(f.posts()).toHaveLength(0)
  })
  it('role revocation aborts a noncooperative POST and scrubs command/body/job', async () => {
    f.server.intercept = r => r.method === 'POST' ? new Promise(() => {}) : null
    prepare(); const pending = f.controller.confirm(); await vi.waitFor(() => expect(f.posts()).toHaveLength(1)); f.update(null); await pending
    expect(f.controller.read()).toMatchObject({ command: null, draft: null, job: null, jobId: null, phase: 'unavailable' })
  })
  it('retains ambiguous identity across event revision changes and allows only read-only recovery in ARCHIVED', async () => {
    f.server.mode = 'lost'; prepare(); await f.controller.confirm(); const key = f.command().key
    f.complete(); f.syncContext(); expect(f.command().key).toBe(key); await f.controller.check()
    expect(f.controller.read()).toMatchObject({ command: null, job: { state: 'COMPLETED' } }); expect(prepare()).toBe(false); expect(f.posts()).toHaveLength(1)
  })
  it('native HTTP timeout leaves one unknown command and no background retry', async () => {
    vi.useFakeTimers(); f.server.intercept = r => r.method === 'POST' ? new Promise(() => {}) : null
    prepare(); const pending = f.controller.confirm(); await vi.advanceTimersByTimeAsync(1001); await pending
    expect(f.controller.read().commandPhase).toBe('unknown'); await vi.advanceTimersByTimeAsync(5000); expect(f.posts()).toHaveLength(1)
  })
})
