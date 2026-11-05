// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { PartyDetail, PartyDiscovery } from '../../ui/PartyDiscovery'
import { activity, ids, party, profile } from '../fixtures/data'
import { harness } from '../fixtures/harness'
import type { Activity } from '../../model/contract'

const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
function setup(detail = false, plan: Activity = activity()) {
  const current = party({ owner: profile(ids.actor) })
  const h = harness(request => {
    const path = new URL(request.url).pathname
    return Response.json(path.endsWith('/me/parties') ? plan
      : path.endsWith('/parties') ? { items: [current], next_cursor: null } : current)
  })
  owned.push(h)
  render(<MemoryRouter>{detail
    ? <PartyDetail controller={h.controller} partyId={ids.party} />
    : <PartyDiscovery controller={h.controller} presentation={{ canCreate: true }} />}</MemoryRouter>)
  return h
}

describe('PI05 discovery and protected navigation', () => {
  it('refreshes through the existing read action, then retains a separate MY PLAN refresh', async () => {
    const h = setup()
    await screen.findByRole('link', { name: party().title })
    const reads = () => h.requests.filter(r => new URL(r.url).pathname.endsWith('/parties') && !new URL(r.url).pathname.endsWith('/me/parties')).length
    const before = reads()
    fireEvent.click(screen.getByRole('button', { name: 'Refresh parties' }))
    await waitFor(() => expect(reads()).toBe(before + 1))
    fireEvent.click(screen.getByRole('button', { name: 'MY PLAN' }))
    expect(screen.queryByRole('button', { name: 'Refresh parties' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Refresh MY PLAN' })).toBeTruthy()
    expect(h.requests.every(request => request.method === 'GET')).toBe(true)
  })

  it('keeps creation explicit and removes owner imagery and destinations on privacy invalidation', async () => {
    const h = setup()
    await screen.findByRole('link', { name: party().title })
    expect(screen.getByRole('link', { name: 'Create party' }).getAttribute('href')).toBe('/meet/parties/new')
    fireEvent.click(screen.getByRole('button', { name: 'Refresh parties' }))
    h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
    await waitFor(() => expect(screen.queryByRole('link', { name: party().title })).toBeNull())
    expect(screen.queryByRole('link', { name: 'Create party' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Create party' }).hasAttribute('disabled')).toBe(true)
    expect(screen.getByText('Event access required')).toBeTruthy()
  })

  it('exposes Manage and Rendezvous only from a current owner projection', async () => {
    const plan = activity()
    setup(true, { ...plan, owned: { items: [party({ owner: profile(ids.actor) })], next_cursor: null } })
    expect((await screen.findByRole('link', { name: 'Manage party' })).getAttribute('href')).toBe(`/meet/parties/${ids.party}/manage`)
    expect(screen.getByRole('link', { name: 'Open rendezvous' }).getAttribute('href')).toBe(`/meet/parties/${ids.party}/rally`)
  })

  it.each(['ACTIVE', 'LEFT', 'REMOVED'] as const)('uses the actual %s membership for Rendezvous visibility', async state => {
    const plan = activity()
    const h = setup(true, { ...plan, memberships: { items: [{ event_id: ids.event, party_id: ids.party, user_id: ids.actor,
      role: 'MEMBER', state, revision: 1, joined_at: '2026-11-03T01:00:00Z' }], next_cursor: null } })
    await screen.findByRole('heading', { name: party().title })
    await waitFor(() => expect(h.controller.read().activity.phase).toBe('ready'))
    expect(screen.queryByRole('link', { name: 'Manage party' })).toBeNull()
    expect(!!screen.queryByRole('link', { name: 'Open rendezvous' })).toBe(state === 'ACTIVE')
  })
})
