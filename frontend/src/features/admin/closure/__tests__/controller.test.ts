import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fixture, ids, job } from '../../mural/__tests__/fixture'
let h: ReturnType<typeof fixture>
beforeEach(() => { vi.stubGlobal('location', new URL('https://mcr.test')); h = fixture(); h.closure.start() })
afterEach(() => { h.cleanup(); vi.unstubAllGlobals() })
describe('FE39 event closure', () => {
  it('review/cancel is read-only; explicit confirm sends one event-revision-bound CLOSE_EVENT', async () => {
    expect(h.closure.prepare('Close after show')).toBe(true); h.closure.cancel(); expect(h.posts()).toHaveLength(0)
    h.closure.prepare('Close after show'); await h.closure.confirm(); await h.closure.confirm()
    expect(h.posts()).toHaveLength(1); expect(await h.posts()[0]!.clone().json()).toEqual({ expected_revision: 8, confirmation: 'CLOSE_EVENT', reason: 'Close after show' })
    expect(h.closure.read().job).toEqual(job()); expect(h.closure.prepare('again')).toBe(false)
  })
  it('shows partial failure, retries the authorized progress read, resumes the same job and preserves completed steps', async () => {
    await h.closure.refresh(ids.job); expect(h.closure.read().job!.steps.find(s => s.name === 'EXPORT')!.state).toBe('FAILED')
    h.server.job = job('RUNNING', 4); await h.closure.refresh(); expect(h.closure.read().job!.state).toBe('RUNNING')
    h.server.job = job('COMPLETED', 5); await h.closure.refresh(); expect(h.closure.read().job!.steps.every(s => s.state === 'SUCCEEDED')).toBe(true)
    expect(h.posts()).toHaveLength(0)
  })
  it.each(['lost', 'pending', 'html'])('%s command is never retried or replaced; recovery binds receipt to authorized job', async mode => {
    h.server.mode = mode; h.closure.prepare('Finish event'); await h.closure.confirm(); const key = h.closure.read().command!.key
    expect(h.closure.prepare('repeat')).toBe(false); await h.closure.confirm(); await h.closure.check()
    expect(h.posts()).toHaveLength(1)
    if (mode === 'lost') expect(h.closure.read()).toMatchObject({ command: null, commandPhase: 'confirmed', jobId: ids.job })
    else expect(h.closure.read().command!.key).toBe(key)
  })
  it('retains unknown across event READ_ONLY revision change and resolves only through exact original status', async () => {
    h.server.mode = 'lost'; h.closure.prepare('Finish event'); await h.closure.confirm(); const key = h.closure.read().command!.key
    h.update({ ...h.context(), phase: 'READ_ONLY', eventRevision: 9 }); expect(h.closure.read().command!.key).toBe(key)
    await h.closure.check(); expect(h.closure.read()).toMatchObject({ command: null, jobId: ids.job }); expect(h.posts()).toHaveLength(1)
  })
  it.each(['DRAFT', 'READ_ONLY', 'ARCHIVED'] as const)('cannot initiate closure in %s', phase => {
    h.update({ ...h.context(), phase }); expect(h.closure.prepare('Finish event')).toBe(false); expect(h.posts()).toHaveLength(0)
  })
  it('role loss and logout scrub job and pending closure; delayed reads cannot resurrect either', async () => {
    await h.closure.refresh(ids.job); h.closure.dispose(); expect(h.closure.read()).toMatchObject({ phase: 'disposed', job: null, jobId: null, command: null })
    h.session.replace(h.session.read().generation, null); await h.closure.refresh(ids.job); expect(h.closure.read().job).toBeNull()
  })
  it('missing role is visibly unavailable; renewed qualified access can reopen controls without a role endpoint', () => {
    const previous = h.context(); h.update(null); h.closure.revalidate(); expect(h.closure.read()).toMatchObject({ phase: 'unavailable' }); expect(h.closure.read().message).toContain('authority unavailable')
    h.update({ ...previous, roleRevision: 2 }); h.closure.revalidate(); expect(h.closure.read().phase).toBe('ready'); expect(h.server.requests).toHaveLength(0)
    expect(h.closure.prepare('Fresh qualified owner')).toBe(true)
  })
  it.each(['wrong-event','wrong-job','extra-private','duplicate-step','false-complete','regress'])('rejects %s closure progress', async mode => {
    await h.closure.refresh(ids.job)
    h.server.transform = value => {
      const p = structuredClone(value) as ClosureRecord
      if (mode === 'wrong-event') p.event_id = ids.restore
      if (mode === 'wrong-job') p.job_id = ids.restore
      if (mode === 'extra-private') p.handles = ['private']
      if (mode === 'duplicate-step') p.steps[6] = p.steps[0]!
      if (mode === 'false-complete') p.state = 'COMPLETED'
      if (mode === 'regress') p.revision = 1
      return p
    }
    await h.closure.refresh(); expect(h.closure.read()).toMatchObject({ phase: 'error', job: null }); expect(h.posts()).toHaveLength(0)
  })
})
interface ClosureRecord { event_id: string; job_id: string; revision: number; state: string; handles?: string[]; steps: { name: string; state: string }[] }
