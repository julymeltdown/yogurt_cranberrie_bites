// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode, useLayoutEffect } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider, useLocation } from 'react-router-dom'
import { PortsProvider } from '../../access'
import { WorldHost } from '../../WorldHost'
import { SocialFeatures, useSocialOwner } from '../provider'
import { SocialRoutePanel, useSocialUnreadTotal } from '../panels'
import { SocialNavigationBoundary } from '../navigation'
import { deferred, fixture, id, post, response, user } from './fixtures'
import type { SocialOwner } from '../owner'
const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).forEach(f => f()) })
function root(path = '/feed', handler?: Parameters<typeof fixture>[0], guard = false) {
  const f = fixture(handler), owners: SocialOwner[] = []
  const scope = { userId: user, eventId: f.scope.eventId!, generation: f.scope.generation }
  const nav = (d: { path: string; scope: typeof f.scope }) => { if (f.auth.privacy.isCurrent(d.scope)) void router.navigate(d.path) }
  function Capture() {
    const owner = useSocialOwner(), total = useSocialUnreadTotal()
    useLayoutEffect(() => { if (owner) owners.push(owner) }, [owner])
    return <output aria-label="Unread total">{total?.total ?? 'unavailable'}</output>
  }
  function Shell() {
    const location = useLocation()
    const content = <><Capture />
      <WorldHost scope={scope} bootstrap={f.bootstrap} privacyScope={f.scope} blocked={location.pathname !== '/world'} covered={false} />
      {location.pathname !== '/world' && <SocialRoutePanel />}
    </>
    return <PortsProvider ports={f.ports}><SocialFeatures {...f.options} navigate={nav}>{guard ? <SocialNavigationBoundary>{content}</SocialNavigationBoundary> : content}</SocialFeatures></PortsProvider>
  }
  const router = createMemoryRouter([{ path: '*', element: <Shell /> }], { initialEntries: [path] })
  const view = render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  stops.push(() => { router.dispose(); f.dispose() })
  const owner = () => owners.at(-1)!
  return { ...f, ...view, router, owners, owner }
}
it('StrictMode releases the discarded owner and retains one world and one active child set across navigation', async () => {
  const f = root(); await screen.findByText(post.text); await waitFor(() => expect(f.owner()).toBeTruthy())
  const owner = f.owner(), host = document.querySelector('[data-world-host]'), mounts = f.stats.mounts
  expect(f.owners.filter(o => !o.isDisposed())).toHaveLength(1)
  await act(() => f.router.navigate('/feed/new')); fireEvent.change(await screen.findByLabelText('Post text'), { target: { value: 'Keep safely across panels' } })
  await act(() => f.router.navigate('/notifications')); await screen.findByText('37 unread in this event')
  expect(screen.getByLabelText('Unread total')).toHaveTextContent('37')
  await act(() => f.router.navigate('/meet/people')); await screen.findByRole('heading', { name: 'PEOPLE' })
  await act(() => f.router.navigate('/feed/new')); expect(await screen.findByLabelText('Post text')).toHaveValue('Keep safely across panels')
  expect(f.owner()).toBe(owner); expect(document.querySelector('[data-world-host]')).toBe(host); expect(f.stats.mounts).toBe(mounts)
  expect(f.stats.peakGames).toBe(1); expect(f.stats.peakConnections).toBe(1)
  f.unmount(); expect(owner.isDisposed()).toBe(true); expect(f.stats.games).toBe(0); expect(f.listenerCount()).toBe(0)
})
it('data-router guard blocks actual links and history POP while a command is unresolved', async () => {
  const f = root('/feed', r => { if (r.method === 'POST') throw new TypeError('uncertain'); return response(r) }, true)
  await screen.findByText(post.text); await act(() => f.router.navigate('/feed/new'))
  fireEvent.change(await screen.findByLabelText('Post text'), { target: { value: 'Retained pending action' } }); fireEvent.click(screen.getByRole('button', { name: 'Publish post' }))
  await screen.findByRole('button', { name: 'Check command status' }); const key = f.owner().thread.read().mutation.key
  await act(() => f.router.navigate(-1)); expect(await screen.findByRole('button', { name: 'Stay with current action' })).toBeVisible()
  expect(screen.getByRole('button', { name: 'Continue navigation' })).toBeDisabled(); expect(f.router.state.location.pathname).toBe('/feed/new')
  expect(f.owner().thread.read().mutation.key).toBe(key); fireEvent.click(screen.getByRole('button', { name: 'Stay with current action' }))
  await act(() => f.router.navigate('/notifications')); expect(await screen.findByRole('button', { name: 'Stay with current action' })).toBeVisible()
  expect(f.router.state.location.pathname).toBe('/feed/new')
})
it('profile direct link uses canonical authorization and leaves sharing/block/provider actions disabled', async () => {
  const f = root(`/people/${user}`); await screen.findByRole('heading', { name: post.author.display_name })
  expect(screen.getByRole('button', { name: 'Request sharing' })).toBeDisabled(); expect(screen.getByRole('button', { name: 'Block user' })).toBeDisabled()
  expect(f.requests.some(r => new URL(r.url).pathname.endsWith(`/people/${user}`))).toBe(true)
  await act(() => f.router.navigate('/me/age')); expect(await screen.findByRole('button', { name: 'Start verification' })).toBeDisabled()
})
it('denied direct profile never shows the prior cached target and removes it from discovery', async () => {
  const f = root('/meet/people', r => new URL(r.url).pathname.includes('/people/') ? Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'private target', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 }) : response(r))
  await screen.findByRole('heading', { name: 'PEOPLE' }); await waitFor(() => expect(f.owner().people.read().page.phase).toBe('ready'))
  await act(() => f.router.navigate(`/people/${user}`)); await screen.findByText('This profile is unavailable.')
  expect(f.owner().people.read().page.data?.items.some(row => row.author.user_id === user)).toBe(false)
})
it('route/back changes cannot replace an unresolved composer; original key/draft stay recoverable', async () => {
  const f = root('/feed/new', r => { if (r.method === 'POST') throw new TypeError('uncertain'); return response(r) })
  fireEvent.change(await screen.findByLabelText('Post text'), { target: { value: 'Pending retained post' } }); fireEvent.click(screen.getByRole('button', { name: 'Publish post' }))
  await screen.findByRole('button', { name: 'Check command status' }); const key = f.owner().thread.read().mutation.key
  await act(() => f.router.navigate('/notifications'))
  expect(screen.getByLabelText('Post text')).toHaveValue('Pending retained post'); expect(screen.queryByRole('heading', { name: 'INBOX' })).not.toBeInTheDocument()
  expect(f.owner().thread.read().mutation.key).toBe(key); expect(screen.getByRole('button', { name: 'Return to current action' })).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Return to current action' })); await waitFor(() => expect(f.router.state.location.pathname).toBe('/feed/new'))
})
it.each(['policy', 'identity'] as const)('withdraws old private DOM and ignores delayed response on %s change', async mode => {
  const late = deferred<Response>(); const f = root(`/feed/${post.post_id}`, r => new URL(r.url).pathname.includes('/posts/') ? late.promise : response(r))
  await waitFor(() => expect(f.requests.some(r => new URL(r.url).pathname.includes('/posts/'))).toBe(true))
  await act(() => { if (mode === 'policy') f.auth.privacy.changePolicy(f.scope, 'consent-revoked'); else f.auth.session.replace(1, { userId: id(900), eventId: f.scope.eventId!, accessToken: 'NEW_SYNTHETIC' }) })
  expect(screen.queryByText(post.text)).not.toBeInTheDocument(); expect(f.owner().isDisposed()).toBe(true)
  await act(async () => { late.resolve(Response.json({ post, comments: [], next_cursor: null })); await Promise.resolve() })
  expect(screen.queryByText(post.text)).not.toBeInTheDocument(); expect(f.stats.games).toBe(0)
})

it('discarding a direct edit ends it once and permits a fresh composer under StrictMode', async () => {
  const f = root(`/feed/${post.post_id}/edit`, undefined, true)
  const text = await screen.findByLabelText('Post text')
  fireEvent.change(text, { target: { value: 'Discard this edit' } })
  fireEvent.click(screen.getByRole('button', { name: 'Discard edit' }))
  await waitFor(() => expect(f.owner().thread.read().draft.edit).toBeNull())
  expect(screen.queryByLabelText('Post text')).not.toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Back to post' })).toBeVisible()
  await act(() => f.router.navigate('/feed/new'))
  expect(await screen.findByLabelText('Post text')).toHaveValue('')
})

it('thread edit button cannot replace a retained new-post draft before navigation', async () => {
  const f = root('/feed/new', undefined, true)
  fireEvent.change(await screen.findByLabelText('Post text'), { target: { value: 'Preserve composer text' } })
  await act(() => f.router.navigate(`/feed/${post.post_id}`))
  fireEvent.click(await screen.findByRole('button', { name: 'Edit post' }))
  expect(f.owner().thread.read().draft).toMatchObject({ text: 'Preserve composer text', edit: null })
  expect(f.router.state.location.pathname).toBe(`/feed/${post.post_id}`)
  await act(() => f.router.navigate('/feed/new'))
  expect(await screen.findByLabelText('Post text')).toHaveValue('Preserve composer text')
})
