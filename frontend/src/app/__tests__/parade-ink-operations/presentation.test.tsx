// @vitest-environment jsdom
// TEST_ONLY isolated controller fixtures; this does not qualify default composition.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { EventOperations } from '../../../features/admin/event/ui/EventOperations'
import { PlacesOperations } from '../../../features/admin/places/PlacesOperations'
import { AuditOperations } from '../../../features/admin/audit/AuditOperations'
import { WallControl } from '../../../features/admin/mural/WallControl'
import { EventClosure } from '../../../features/admin/closure/EventClosure'
import { MemberReviewScreen } from '../../../features/admin/members'
import { ContentReviewScreen } from '../../../features/admin/content'
import { admission } from '../../../features/auth/__tests__/enrollment-consent/fixtures'
import { fixture as operations } from '../../../features/admin/event/__tests__/fixtures/data'
import { fixture as mural } from '../../../features/admin/mural/__tests__/fixture'
import { harness as moderation } from '../../../features/admin/reports/__tests__/fixtures/harness'

const dispose: (() => void)[] = []
beforeEach(() => { vi.stubGlobal('location', new URL('https://mcr.example.invalid')) })
afterEach(() => { cleanup(); dispose.splice(0).forEach(fn => fn()); vi.unstubAllGlobals() })
// Valid HTTP fixture only; canonical Auth still decodes the admission response.
function admissionResponse(request: Request, token: string | null) {
  return request.method === 'GET' && new URL(request.url).pathname === '/api/auth/enrollment/admission'
    && token && request.headers.get('Authorization') === `Bearer ${token}` ? Response.json(admission()) : null
}
function eventFixture() {
  const f = operations(); dispose.push(f.dispose)
  f.intercept(request => admissionResponse(request, f.auth.session.read().accessToken))
  return f
}
function wallFixture() { const f = mural(); dispose.push(f.cleanup); return f }

describe('PI12 presentation preserves controller boundaries', () => {
  it.each([true, false])('retains the event review barrier and cancellation (summary=%s)', async summaryOnly => {
    const f = eventFixture()
    const { container } = render(<EventOperations controller={f.controller} summaryOnly={summaryOnly} />)
    await screen.findByText('SYNTHETIC operations event')
    expect(container.querySelector('[data-operations-view]')?.getAttribute('data-operations-view')).toBe(summaryOnly ? 'control' : 'event')
    fireEvent.change(screen.getByLabelText('Reason for settings or phase change'), { target: { value: 'Reviewed synthetic settings' } })
    fireEvent.click(screen.getByRole('button', { name: 'Review settings change' }))
    expect(screen.getByRole('button', { name: 'Confirm this change' })).toBeTruthy()
    expect(f.writes()).toHaveLength(0)
    fireEvent.click(screen.getByRole('button', { name: 'Cancel confirmation' }))
    expect(f.writes()).toHaveLength(0)
    expect(screen.queryByRole('button', { name: 'Confirm this change' })).toBeNull()
  })

  it('keeps place source review focused and clears it when authority is withdrawn', async () => {
    const f = eventFixture()
    render(<PlacesOperations controller={f.controller} />)
    fireEvent.click(await screen.findByRole('button', { name: 'Review SYNTHETIC landmark' }))
    const heading = screen.getByRole('heading', { name: 'Review place source' })
    expect(document.activeElement).toBe(heading)
    expect(screen.getByText('https://venue.example.invalid/review')).toBeTruthy()
    expect(f.writes()).toHaveLength(0)
    act(() => f.setLease(null))
    await screen.findByRole('heading', { name: 'Operations unavailable' })
    expect(screen.queryByRole('heading', { name: 'Review place source' })).toBeNull()
  })

  it('retains minimal audit metadata and the keyboard reachable scrolling region', async () => {
    const f = eventFixture()
    render(<AuditOperations controller={f.controller} />)
    await screen.findByRole('button', { name: 'View audit entry' })
    expect(screen.getByRole('region', { name: 'Scrollable action outcomes' }).tabIndex).toBe(0)
    expect(screen.queryByText('Raw sensitive failure text must never render')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Search audit log' }))
    await waitFor(() => expect(f.requests.filter(r => new URL(r.url).pathname.endsWith('/audit')).length).toBe(2))
    expect(f.writes()).toHaveLength(0)
  })

  it('keeps Wall confirmation separate from dispatch and preserves an uncertain command', async () => {
    const f = wallFixture(); f.server.mode = 'lost'
    await f.mural.start()
    render(<WallControl controller={f.mural} />)
    fireEvent.change(screen.getByLabelText('Reason'), { target: { value: 'Synthetic pause review' } })
    fireEvent.click(screen.getByRole('button', { name: 'Review wall action' }))
    expect(document.activeElement).toBe(screen.getByRole('group', { name: 'Confirm wall action' }))
    expect(f.posts()).toHaveLength(0)
    fireEvent.click(screen.getByRole('button', { name: 'Confirm wall action' }))
    await screen.findByRole('button', { name: 'Check original wall command' })
    expect(f.posts()).toHaveLength(1)
    expect(screen.queryByRole('button', { name: 'Review wall action' })).toBeNull()
  })

  it('keeps closure cancellation local and its existing-job observation reachable', () => {
    const f = wallFixture()
    render(<EventClosure controller={f.closure} />)
    fireEvent.change(screen.getByLabelText('Closure reason'), { target: { value: 'Synthetic closure review' } })
    fireEvent.click(screen.getByRole('button', { name: 'Review event closure' }))
    expect(screen.getByRole('group', { name: 'Confirm event closure' })).toBeTruthy()
    expect(f.posts()).toHaveLength(0)
    fireEvent.click(screen.getByRole('button', { name: 'Cancel event closure' }))
    expect(screen.getByRole('button', { name: 'Load existing closure job' })).toBeTruthy()
    expect(f.posts()).toHaveLength(0)
  })

  it.each(['members', 'content'] as const)('keeps %s confirmation, focus and privacy under the original moderation view', async lane => {
    const f = moderation(lane); dispose.push(f.dispose)
    f.override(request => admissionResponse(request, f.auth.session.read().accessToken))
    const View = lane === 'members' ? MemberReviewScreen : ContentReviewScreen
    const { container } = render(<View controller={f.controller} />)
    fireEvent.click(await screen.findByRole('button', { name: /^Review / }))
    fireEvent.change(screen.getByLabelText('Action'), { target: { value: lane === 'members' ? 'SUSPEND' : 'HIDE' } })
    fireEvent.change(screen.getByLabelText('Reason for this action'), { target: { value: 'Synthetic factual review' } })
    const opener = screen.getByRole('button', { name: 'Review action' })
    fireEvent.click(opener)
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Confirm action' }))
    expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(0)
    fireEvent.click(screen.getByRole('button', { name: 'Cancel confirmation' }))
    expect(document.activeElement).toBe(opener)
    expect(container.querySelectorAll('main')).toHaveLength(1)
    act(() => f.setGrant(null))
    await screen.findByRole('heading', { name: 'Moderator access unavailable' })
    expect(screen.queryByLabelText('Reason for this action')).toBeNull()
    expect(screen.queryByText('Private test statement')).toBeNull()
    expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(0)
  })
})
