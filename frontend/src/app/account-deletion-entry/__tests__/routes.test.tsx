// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { createFixturePorts } from '../../__tests__/fixtures/ports'
import { fixture } from '../../../features/auth/__tests__/fixtures/data'
import { accepted, fixtureAuth, otherId, saga } from '../../../features/auth/__tests__/account-deletion/fixtures'
const owned: (() => void)[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(f => f()) })
async function setup(mode: 'pending' | 'uncertain' | 'legacy' = 'pending', path = '/me/settings') {
  let currentMode: string = mode
  const h = fixtureAuth(r => {
    const p = new URL(r.url).pathname
    if (p.endsWith('/bootstrap')) { const b = fixture(); return Response.json({ ...b, capabilities: { ...b.capabilities, world: { available: true, reason: null } } }) }
    if (p.endsWith('/me')) return Response.json(fixture().participant)
    if (p === '/api/auth/account' && r.method === 'DELETE') {
      if (currentMode === 'uncertain') return Response.json({ code: 'PRIVATE', message: 'PRIVATE failure' }, { status: 503 })
      const s = saga(); if (currentMode === 'legacy') s.steps = s.steps.slice(0, 4)
      return Response.json(accepted(s), { status: 202 })
    }
    if (p === '/api/auth/account/deletion') return Response.json(saga(currentMode === 'complete'))
    return Response.json({ code: 'UNAVAILABLE' }, { status: 503 })
  })
  await h.login()
  const world = createFixturePorts()
  const router = createMemoryRouter(createAppRoutes({ ...world.ports, auth: h.auth, session: h.auth.session, readiness: h.auth.readiness, bootstrap: { load: h.auth.loadBootstrap } }), { initialEntries: [path] })
  owned.push(() => { router.dispose(); h.dispose() })
  render(<StrictMode><RouterProvider router={router}/></StrictMode>)
  return { ...h, ...world, router, mode(value: string) { currentMode = value } }
}
async function request() {
  fireEvent.click(await screen.findByRole('button', { name: 'Request account deletion' }))
  fireEvent.change(screen.getByLabelText('Deletion confirmation'), { target: { value: 'DELETE_MY_ACCOUNT' } })
  fireEvent.click(screen.getByRole('button', { name: 'Confirm deletion request' }))
  await screen.findByRole('heading', { name: 'Account deletion status' })
}
it('default Settings reaches account controls and Back restores opener without replacing the world', async () => {
  const h = await setup(); const opener = await screen.findByRole('link', { name: 'Account controls' }), host = document.querySelector('[data-world-host]')
  const mounts = h.stats.mounts; fireEvent.click(opener)
  await screen.findByRole('heading', { name: 'Account controls' }); expect(h.router.state.location.pathname).toBe('/me/account')
  fireEvent.click(screen.getByRole('button', { name: /^Back$/ }))
  await screen.findByRole('heading', { name: 'SETTINGS' }); await waitFor(() => expect(screen.getByRole('link', { name: 'Account controls' })).toHaveFocus())
  expect(document.querySelector('[data-world-host]')).toBe(host); expect(h.stats.mounts).toBe(mounts); expect(h.stats.peakConnections).toBe(1)
})
it.each(['pending', 'uncertain', 'legacy'] as const)('%s survives WorldShell disposal and manually recovers to all-eight completion', async mode => {
  const h = await setup(mode, '/me/account'); await screen.findByRole('heading', { name: 'Account controls' })
  h.auth.queryClient.setQueryData(['private', 'fixture'], 'PRIVATE')
  await request()
  expect(h.router.state.location.pathname).toBe('/account/deleted'); expect(h.auth.privacy.capture().gate).toBe('suspended')
  expect(h.auth.queryClient.getQueryData(['private', 'fixture'])).toBeUndefined(); expect(document.querySelector('[data-world-host]')).toBeNull(); expect(h.stats.games).toBe(0)
  const deletes = () => h.requests.filter(r => r.method === 'DELETE'); expect(deletes()).toHaveLength(1)
  expect(h.requests.filter(r => r.url.endsWith('/account/deletion'))).toHaveLength(0)
  if (mode === 'pending') expect(screen.getAllByRole('listitem')).toHaveLength(8)
  else expect(screen.queryByRole('list', { name: 'Deletion progress' })).toBeNull()
  h.mode('complete'); fireEvent.click(screen.getByRole('button', { name: 'Check deletion progress' }))
  await screen.findByText('Account deletion confirmed by the Auth owner.')
  expect(h.auth.session.read().userId).toBeNull(); expect(deletes()).toHaveLength(1)
  expect(h.requests.filter(r => r.url.endsWith('/account/deletion'))).toHaveLength(1)
  expect(h.requests.some(r => r.url.endsWith('/refresh'))).toBe(false); expect(h.stats.games).toBe(0)
})
it('public direct entry has no fabricated completion, private owner or automatic status read', async () => {
  const h = await setup('pending', '/account/deleted')
  expect(await screen.findByRole('heading', { name: 'Account deletion status' })).toHaveFocus()
  expect(await screen.findByText(/No deletion completion is confirmed/)).toBeVisible()
  expect(screen.queryByRole('button', { name: 'Check deletion progress' })).toBeNull(); expect(h.stats.mounts).toBe(0)
  expect(h.requests.filter(r => /\/account(?:\/deletion)?$/.test(new URL(r.url).pathname))).toHaveLength(0)
})
it('a new actor clears retained public progress and cannot receive an old recovery read', async () => {
  const h = await setup('pending', '/me/account'); await request()
  await act(async () => { h.auth.session.replace(h.auth.session.read().generation, { userId: otherId, eventId: h.auth.session.read().eventId, accessToken: 'NEXT' }) })
  expect(await screen.findByText(/No deletion completion is confirmed/)).toBeVisible()
  expect(screen.queryByRole('list', { name: 'Deletion progress' })).toBeNull(); expect(screen.queryByRole('button', { name: 'Check deletion progress' })).toBeNull()
  expect(h.requests.filter(r => r.url.endsWith('/account/deletion'))).toHaveLength(0)
})
