// @vitest-environment jsdom
import { act, cleanup, render } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import type { SocialOwner } from '../../social-composition/owner'
import type { FeedState } from '../../../features/event-feed/controller'
import { useLoadedProfileActivity } from '../activity'

const holder = vi.hoisted(() => ({ owner: null as SocialOwner | null }))
vi.mock('../../social-composition/provider', () => ({ useSocialOwner: () => holder.owner }))
afterEach(() => { cleanup(); holder.owner = null })
it('observes only an existing authorized ready feed page without starting a request', () => {
  const listeners = new Set<() => void>(), request = vi.fn(), start = vi.fn()
  const scope = { userId: '11111111-1111-4111-8111-111111111111', eventId: '22222222-2222-4222-8222-222222222222', generation: 1, policyGeneration: 0, gate: 'open' as const }
  let current = true, available = true
  let state: FeedState = { filter: 'ALL', available: true, readOnly: false,
    feed: { phase: 'idle', data: null, failure: null }, notices: { phase: 'idle', data: null, failure: null }, notice: { phase: 'idle', data: null, failure: null } }
  holder.owner = { scope, isDisposed: () => false, available: () => available, auth: { request, privacy: { isCurrent: () => current } },
    feed: { read: () => state, start, subscribe: (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener) } } } } as unknown as SocialOwner
  function Probe() { const activity = useLoadedProfileActivity(scope.userId); return <p>{activity ? `${activity.subjectUserId}:${activity.page.items.length}` : 'Unavailable'}</p> }
  const view = render(<Probe />)
  expect(view.container.textContent).toBe('Unavailable'); expect(start).not.toHaveBeenCalled(); expect(request).not.toHaveBeenCalled()
  act(() => { state = { ...state, feed: { phase: 'ready', data: { items: [], next_cursor: null }, failure: null } }; listeners.forEach(fn => fn()) })
  expect(view.container.textContent).toBe(`${scope.userId}:0`)
  act(() => { available = false; state = { ...state }; listeners.forEach(fn => fn()) })
  expect(view.container.textContent).toBe('Unavailable')
  act(() => { available = true; current = false; state = { ...state }; listeners.forEach(fn => fn()) })
  expect(view.container.textContent).toBe('Unavailable'); expect(start).not.toHaveBeenCalled(); expect(request).not.toHaveBeenCalled()
})
