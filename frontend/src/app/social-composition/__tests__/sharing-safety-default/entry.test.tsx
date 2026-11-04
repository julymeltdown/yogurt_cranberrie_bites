// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, configure, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../../router'
import { fixture, event, user, post } from '../fixtures'
import { scenarioService } from './service'
import type { SocialContext } from '../../ports'
configure({ asyncUtilTimeout: 1500 })
const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()) })
function setup(path = '/meet/people', mode = 'valid') {
  const service = scenarioService(user, event); service.mode(mode)
  const f = fixture(r => service.respond(r))
  const ports = { ...f.ports, bootstrap: { load: async () => f.bootstrap }, social: { availability(context: SocialContext) {
    return { read: () => ({ ...context, services: { people: true, requests: true, feed: true } }), subscribe: () => () => {} }
  } } }
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/me', path] })
  render(<RouterProvider router={router} />); stops.push(() => { router.dispose(); f.dispose() })
  return { ...f, service, router }
}
const writes = (f: ReturnType<typeof setup>) => f.requests.filter(r => r.method !== 'GET')
async function profile(f: ReturnType<typeof setup>) {
  await screen.findByRole('heading', { name: 'PEOPLE' })
  await act(async () => { await f.router.navigate(`/people/${f.service.recipient}`) })
  return screen.findByRole('button', { name: 'Request sharing' })
}
it('opens actual default Profile → bound offer, requires explicit platform consent and dispatches one private request', async () => {
  const f = setup(), share = await profile(f), mounts = f.stats.mounts
  await waitFor(() => expect(share).toBeEnabled()); fireEvent.click(share)
  const send = await screen.findByRole('button', { name: 'Send sharing request' }); expect(send).toBeDisabled()
  expect(writes(f)).toHaveLength(0)
  fireEvent.click(screen.getByRole('checkbox', { name: /I consent to share my Instagram/ })); fireEvent.click(send)
  await screen.findByText('The server confirmed this change.')
  expect(writes(f)).toHaveLength(1)
  expect(await writes(f)[0]!.json()).toEqual({ recipient_id: f.service.recipient, intro_id: f.service.intro.intro_id, intro_revision: f.service.intro.revision,
    consents: [{ platform: 'INSTAGRAM', requester_handle_version: 2, recipient_handle_version: 3, requester_consents: true }] })
  expect(f.requests.some(r => r.url.endsWith(`/intros/${f.service.intro.intro_id}`))).toBe(true)
  expect(f.requests.some(r => r.url.includes('/reveal'))).toBe(false); expect(f.stats.mounts).toBe(mounts)
  expect(f.auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
})
it.each(['wrong-recipient', 'wrong-revision'])('does not expose an actionable %s offer', async mode => {
  const f = setup('/meet/people', mode); fireEvent.click(await profile(f))
  await screen.findByText('This introduction changed. Return to the current profile before requesting sharing.')
  expect(screen.queryByRole('button', { name: 'Send sharing request' })).not.toBeInTheDocument(); expect(writes(f)).toHaveLength(0)
})
it('retains the original uncertain sharing command across blocked navigation and manual recovery', async () => {
  const f = setup(); fireEvent.click(await profile(f)); const consent = await screen.findByRole('checkbox', { name: /I consent to share my Instagram/ })
  f.service.mode('unknown'); fireEvent.click(consent); fireEvent.click(screen.getByRole('button', { name: 'Send sharing request' }))
  await screen.findByRole('button', { name: 'Check change status' }); const key = writes(f)[0]!.headers.get('Idempotency-Key')
  fireEvent.click(screen.getByRole('link', { name: 'Social links' })); await screen.findByRole('button', { name: 'Stay with current action' })
  expect(f.router.state.location.pathname).toBe('/me/requests'); fireEvent.click(screen.getByRole('button', { name: 'Stay with current action' }))
  fireEvent.click(screen.getByRole('button', { name: 'Check change status' }))
  await waitFor(() => expect(f.requests.some(r => new URL(r.url).pathname.endsWith('/commands'))).toBe(true))
  const url = new URL(f.requests.find(r => new URL(r.url).pathname.endsWith('/commands'))!.url)
  expect(url.searchParams.get('idempotency_key')).toBe(key); expect(url.searchParams.get('operation_id')).toBe('createConcertSocialRequest'); expect(writes(f)).toHaveLength(1)
})
it('keeps sharing unavailable without a current introduction and fails closed on its fresh denial', async () => {
  const f = setup('/meet/people', 'no-intro'), share = await profile(f); expect(share).toBeDisabled()
  expect(writes(f)).toHaveLength(0)
})
it('mounts actual USER report and blocks routes, rechecks target on submit, and retains the original report key across navigation', async () => {
  const f = setup(`/report?targetType=USER&targetId=00000000-0000-4000-8000-000000000099`)
  const submit = await screen.findByRole('button', { name: 'Submit report' }); f.service.mode('unknown'); fireEvent.click(submit)
  await screen.findByRole('button', { name: 'Check original request status' }); const key = writes(f)[0]!.headers.get('Idempotency-Key')
  expect(f.requests.filter(r => r.url.includes(`/people/${f.service.recipient}`))).toHaveLength(2)
  fireEvent.click(screen.getByRole('button', { name: 'Close report' })); await act(async () => { await f.router.navigate('/me/blocks') })
  await screen.findByRole('region', { name: 'Blocked users' }); fireEvent.click(await screen.findByRole('button', { name: 'Check original request status' }))
  await waitFor(() => expect(f.requests.some(r => new URL(r.url).searchParams.get('idempotency_key') === key)).toBe(true))
  expect(writes(f)).toHaveLength(1); expect(screen.queryByRole('button', { name: 'Submit report' })).not.toBeInTheDocument()
})
it('requires Profile block confirmation, clears old privacy views, lists and confirms unblock without restoring consent', async () => {
  const f = setup(); await profile(f); fireEvent.click(screen.getByRole('button', { name: 'Block user' }))
  const confirm = await screen.findByRole('button', { name: 'Confirm block' }); expect(writes(f)).toHaveLength(0); fireEvent.click(confirm)
  await waitFor(() => expect(f.auth.privacy.capture().policyGeneration).toBeGreaterThan(f.scope.policyGeneration))
  await screen.findByRole('region', { name: 'Blocked users' }); expect(f.service.blocked()).toBe(true)
  fireEvent.click(await screen.findByRole('button', { name: 'Unblock user' })); fireEvent.click(await screen.findByRole('button', { name: 'Confirm unblock' }))
  await screen.findByText('No blocked users on this page.'); expect(f.service.blocked()).toBe(false)
  expect(writes(f).map(r => r.method)).toEqual(['PUT', 'DELETE']); expect(f.auth.privacy.capture().policyGeneration).toBe(f.scope.policyGeneration + 2)
  expect(f.requests.some(r => /reveal|social-requests/.test(r.url))).toBe(false)
})
it('keeps denied own safety reads unavailable rather than inventing readiness', async () => {
  const f = setup('/me/blocks', 'blocks-denied')
  await screen.findByText('This safety service is unavailable. No action has been sent.')
  expect(screen.queryByRole('button', { name: 'Unblock user' })).not.toBeInTheDocument(); expect(writes(f)).toHaveLength(0)
})
it('mounts the existing Thread POST target through the default report route and registered fresh target reader', async () => {
  const f = setup(`/report?targetType=POST&targetId=${post.post_id}`)
  fireEvent.click(await screen.findByRole('button', { name: 'Submit report' }))
  await screen.findByText('Report submitted. Only authorized reviewers can access its details.')
  expect(f.requests.filter(r => new URL(r.url).pathname.endsWith(`/posts/${post.post_id}`))).toHaveLength(2)
  expect(await writes(f)[0]!.json()).toMatchObject({ target_type: 'POST', target_id: post.post_id, evidence_media_ids: [] })
})
