// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter, useLocation } from 'react-router-dom'
import { PartyDetail } from '../../ui/PartyDiscovery'
import type { Activity, Member } from '../../model/contract'
import { activity, ids, party, profile } from '../fixtures/data'
import { harness } from '../fixtures/harness'

const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
const member = (state: Member['state'] = 'ACTIVE'): Member => ({ event_id: ids.event, party_id: ids.party, user_id: ids.actor,
  role: 'MEMBER', state, revision: 1, joined_at: '2026-11-01T01:00:00Z' })
function Location() { return <output aria-label="Current panel">{useLocation().pathname}</output> }
function setup(plan: Activity, owner = false) {
  const current = party(owner ? { owner: profile(ids.actor) } : {})
  const h = harness(request => {
    const path = new URL(request.url).pathname
    return Response.json(path.endsWith('/me/parties') ? plan : path.endsWith('/parties') ? { items: [], next_cursor: null } : current)
  }); owned.push(h)
  render(<MemoryRouter initialEntries={[`/meet/parties/${ids.party}`]}><PartyDetail controller={h.controller} partyId={ids.party} /><Location /></MemoryRouter>)
  return h
}

describe('IN02 same-shell party entry points', () => {
  it('opens management and rendezvous for decoded current ownership without applying', async () => {
    const plan = { ...activity(), owned: { items: [party({ owner: profile(ids.actor) })], next_cursor: null } }
    const h = setup(plan, true)
    const manage = await screen.findByRole('link', { name: 'Manage party' })
    expect(h.auth.read().admission).toBe('ADMITTED')
    expect(h.admissionRequests).toHaveLength(1)
    expect(h.admissionRequests[0]?.method).toBe('GET')
    expect(manage.getAttribute('href')).toBe(`/meet/parties/${ids.party}/manage`)
    expect(screen.getByRole('link', { name: 'Open rendezvous' }).getAttribute('href')).toBe(`/meet/parties/${ids.party}/rally`)
    const session = h.auth.session.read()
    fireEvent.click(manage)
    expect(screen.getByLabelText('Current panel').textContent).toBe(`/meet/parties/${ids.party}/manage`)
    expect(h.auth.session.read()).toBe(session)
    expect(h.requests.every(request => request.method === 'GET')).toBe(true)
  })

  it('opens rendezvous for an active member without exposing owner management', async () => {
    setup({ ...activity(), memberships: { items: [member()], next_cursor: null } })
    const rally = await screen.findByRole('link', { name: 'Open rendezvous' })
    expect(screen.queryByRole('link', { name: 'Manage party' })).toBeNull()
    fireEvent.click(rally)
    expect(screen.getByLabelText('Current panel').textContent).toBe(`/meet/parties/${ids.party}/rally`)
  })

  it.each(['REMOVED', 'LEFT'] as const)('hides both controls for %s membership', async state => {
    const h = setup({ ...activity(), memberships: { items: [member(state)], next_cursor: null } })
    await waitFor(() => expect(h.controller.read().activity.phase).toBe('ready'))
    await screen.findByRole('heading', { name: party().title })
    expect(screen.queryByRole('link', { name: 'Open rendezvous' })).toBeNull()
    expect(screen.queryByRole('link', { name: 'Manage party' })).toBeNull()
  })

  it('does not infer membership from public party details alone', async () => {
    const h = setup(activity())
    await waitFor(() => expect(h.controller.read().activity.phase).toBe('ready'))
    await screen.findByRole('heading', { name: party().title })
    expect(screen.queryByRole('link', { name: 'Open rendezvous' })).toBeNull()
    expect(screen.queryByRole('link', { name: 'Manage party' })).toBeNull()
  })

  it('withdraws mounted membership links synchronously when current privacy access changes', async () => {
    const h = setup({ ...activity(), memberships: { items: [member()], next_cursor: null } })
    await screen.findByRole('link', { name: 'Open rendezvous' })
    act(() => h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block'))
    expect(screen.queryByRole('link', { name: 'Open rendezvous' })).toBeNull()
    expect(screen.queryByRole('link', { name: 'Manage party' })).toBeNull()
  })
})
