// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { fixture, user, event } from '../../social-composition/__tests__/fixtures'
import type { SocialContext } from '../../social-composition/ports'
import { privateService } from '../../../../e2e/visual/parade-ink-private-pages/service'
import { createPolicyTransport } from '../../pre-world-policy/transport'
import { policyFixtureRelease } from '../../pre-world-policy/__tests__/fixtures'
import { createProductionPorts } from '../../ports'
const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); vi.restoreAllMocks(); vi.unstubAllGlobals() })
async function setup(path: string) {
  const service = privateService(user, event), f = fixture(r => service.respond(r))
  if (!await f.auth.checkAdmission()) throw new Error('Controlled canonical admission unavailable')
  const ports = { ...f.ports, preWorldPolicy: createPolicyTransport(f.auth, () => policyFixtureRelease), bootstrap: { load: async () => f.bootstrap }, social: { availability(context: SocialContext) {
    return { read: () => ({ ...context, services: { people: true, requests: true, age: true, inbox: true, feed: true } }), subscribe: () => () => {} }
  } } }
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: [path] })
  render(<RouterProvider router={router} />); stops.push(() => { router.dispose(); f.dispose() })
  return { ...f, service, router }
}
it('default Social links keeps names private, validates input, and binds removal to explicit current confirmation', async () => {
  const f = await setup('/me/social')
  const input = await screen.findByLabelText('Replacement Instagram account name')
  expect(input.closest('.pi-private-social')).not.toBeNull()
  expect(document.body.textContent).not.toContain('@controlled_fixture')
  fireEvent.change(input, { target: { value: 'bad..name' } }); expect(screen.getByRole('button', { name: 'Replace privately' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Remove Instagram' }))
  expect(f.requests.filter(r => r.method !== 'GET')).toHaveLength(0)
  fireEvent.click(screen.getByRole('button', { name: 'Keep handle' }))
  expect(screen.queryByRole('button', { name: 'Confirm removal' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Remove Instagram' })); fireEvent.click(screen.getByRole('button', { name: 'Confirm removal' }))
  await waitFor(() => expect(f.requests.filter(r => r.method === 'DELETE')).toHaveLength(1))
  expect(new URL(f.requests.find(r => r.method === 'DELETE')!.url).searchParams.get('expected_revision')).toBe('2')
  expect(f.requests.some(r => r.url.includes('/reveal'))).toBe(false)
})
it('default Requests preserves explicit reciprocal consent, approved metadata and secret cleanup on return', async () => {
  const f = await setup('/me/requests')
  fireEvent.click(await screen.findByRole('button', { name: 'Open request' }))
  const approve = await screen.findByRole('button', { name: 'Approve selected platforms' }); expect(approve).toBeDisabled()
  fireEvent.click(screen.getByRole('checkbox', { name: 'Exchange Instagram account names' })); expect(approve).toBeDisabled()
  fireEvent.click(screen.getByRole('checkbox', { name: /I explicitly consent/ })); fireEvent.click(approve)
  await screen.findByRole('button', { name: 'Reveal Instagram' })
  expect(f.requests.some(r => r.url.includes('/reveal'))).toBe(false)
  fireEvent.click(screen.getByRole('button', { name: 'Reveal Instagram' }))
  await screen.findByText('@controlled_fixture')
  const secret = f.requests.find(r => new URL(r.url).pathname.endsWith('/reveal'))!
  expect(secret.cache).toBe('no-store')
  fireEvent.click(screen.getByRole('button', { name: 'Back to requests' }))
  expect(screen.queryByText('@controlled_fixture')).toBeNull()
  expect(f.auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
})
it('default Age & privacy reflects real decoded expiry and preserves the unavailable provider action', async () => {
  const f = await setup('/me/age')
  await screen.findByRole('heading', { name: 'Not verified' }); expect(screen.getByRole('button', { name: 'Start verification' })).toBeDisabled()
  f.service.age('EXPIRED'); fireEvent.click(screen.getByRole('button', { name: 'Refresh age status' }))
  await screen.findByRole('heading', { name: 'Verification expired' })
  expect(f.requests.filter(r => r.method !== 'GET')).toHaveLength(0)
})
it('default Inbox confirms one read-state command and keeps an inaccessible target safe', async () => {
  const f = await setup('/notifications')
  fireEvent.click(await screen.findByRole('button', { name: 'Mark as read' }))
  await screen.findByText('Read-state command confirmed.')
  expect(f.requests.filter(r => r.method === 'PATCH')).toHaveLength(1)
  expect(await f.requests.find(r => r.method === 'PATCH')!.json()).toEqual({ expected_revision: 1 })
  fireEvent.click(screen.getByRole('button', { name: 'Open update' }))
  await waitFor(() => expect(f.router.state.location.pathname).toBe('/me/requests'))
  expect(document.body.textContent).not.toContain('@controlled_fixture')
})
it('default Blocked users retains cancellation, one confirmed unblock and no restored consent', async () => {
  const f = await setup('/me/blocks')
  fireEvent.click(await screen.findByRole('button', { name: 'Unblock user' }))
  expect(await screen.findByRole('dialog', { name: 'Unblock this user?' })).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Keep blocked' }))
  expect(f.requests.filter(r => r.method !== 'GET')).toHaveLength(0)
  fireEvent.click(screen.getByRole('button', { name: 'Unblock user' })); fireEvent.click(screen.getByRole('button', { name: 'Confirm unblock' }))
  await screen.findByText('No blocked users on this page.')
  expect(f.requests.filter(r => r.method === 'DELETE')).toHaveLength(1)
  expect(f.requests.some(r => r.url.includes('/reveal'))).toBe(false)
})
it('actual public Archive factory reads without credentials and keeps canvas, pagination and unavailable media truthful', async () => {
  const service = privateService(user, event), requests: Request[] = []
  vi.stubGlobal('location', { origin: 'https://mcr.example' })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ createImageData: () => ({ data: new Uint8ClampedArray(128 * 128 * 4) }), putImageData: vi.fn(), clearRect: vi.fn() } as unknown as CanvasRenderingContext2D)
  vi.stubGlobal('fetch', async (r: Request) => { requests.push(r); return service.respond(r) })
  const ports = createProductionPorts(undefined, {}, undefined, event)
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/archive'] })
  render(<RouterProvider router={router} />); stops.push(() => { router.dispose(); ports.dispose?.() })
  await screen.findByText(/We sang every word/)
  fireEvent.click(screen.getByRole('button', { name: /View archived image 1/ }))
  await screen.findByText('Archived image unavailable. No live or private image is substituted.')
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Next page' })) })
  await screen.findByText('The last chorus stayed with us.')
  expect(screen.getByRole('heading', { name: 'Posts after the show' })).toHaveFocus()
  for (const r of requests) { expect(r.method).toBe('GET'); expect(r.credentials).toBe('omit'); expect(r.cache).toBe('no-store'); expect(r.headers.has('Authorization')).toBe(false) }
  expect(screen.queryByRole('textbox')).toBeNull(); expect(document.querySelector('[data-world-host]')).toBeNull()
})
