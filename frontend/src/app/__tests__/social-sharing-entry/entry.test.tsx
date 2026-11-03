// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../../router'
import { fixture, response, user, event } from '../../social-composition/__tests__/fixtures'
import type { SocialAvailability, SocialContext } from '../../social-composition/ports'
import type { ShellBootstrap } from '../../ports'
import type { SocialOptions } from '../../social-composition/ports'
import { sharingService } from './service'
const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()) })
function setup(path = '/me/social', ready = true, readOnly = false, capability = true, requests?: SocialOptions['requests']) {
  const service = sharingService(user, event)
  const f = fixture(async r => await service.respond(r) ?? response(r))
  let enabled = ready, factories = 0
  const listeners = new Set<() => void>()
  let bootstrap: ShellBootstrap = { ...f.bootstrap, event: { ...f.bootstrap.event, phase: readOnly ? 'READ_ONLY' as const : 'OPEN' as const },
    capabilities: { ...f.bootstrap.capabilities,
      ...(!capability ? { social_sharing: { available: false as const, reason: 'FEATURE_DISABLED' as const } } : {}),
      ...(readOnly ? { mutations: { available: false as const, reason: 'EVENT_READ_ONLY' as const } } : {}) } }
  const ports = { ...f.ports, bootstrap: { load: async () => bootstrap }, social: {
    ...(requests ? { requests } : {}),
    availability(context: SocialContext) {
      factories++
      return { read: (): SocialAvailability => ({ ...context, services: { requests: enabled } }), subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } } }
    },
  } }
  const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/me', path] })
  const mounted = render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  stops.push(() => { router.dispose(); f.dispose() })
  return { ...f, service, router, factories: () => factories, unmount: mounted.unmount,
    readOnly() { bootstrap = { ...bootstrap, event: { ...bootstrap.event, phase: 'READ_ONLY' }, capabilities: { ...bootstrap.capabilities, mutations: { available: false, reason: 'EVENT_READ_ONLY' } } }; f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'revalidated') },
    ready(value: boolean) { enabled = value; listeners.forEach(fn => fn()) } }
}
async function detail() { fireEvent.click(await screen.findByRole('button', { name: 'Open request' })); return screen.findByRole('button', { name: 'Reveal Instagram' }) }
const writes = (f: ReturnType<typeof setup>) => f.requests.filter(r => r.method !== 'GET')
it('mounts actual declared handles/requests/detail and retains drafts, host and service owner without Query entries', async () => {
  const f = setup(); const input = await screen.findByLabelText('Replacement Instagram account name')
  const host = document.querySelector('[data-world-host]'), factories = f.factories(), mounts = f.stats.mounts
  fireEvent.change(input, { target: { value: 'private_draft' } })
  fireEvent.click(screen.getByRole('link', { name: 'Requests' })); await detail()
  expect(f.requests.filter(r => r.url.includes('/reveal'))).toHaveLength(0)
  fireEvent.click(screen.getByRole('button', { name: 'Reveal Instagram' })); await screen.findByText(`@${f.service.secret}`)
  fireEvent.click(screen.getByRole('button', { name: 'Hide account name' })); expect(screen.queryByText(`@${f.service.secret}`)).not.toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Back to requests' })); await screen.findByRole('button', { name: 'Open request' })
  fireEvent.click(screen.getByRole('link', { name: 'Social links' })); expect(await screen.findByLabelText('Replacement Instagram account name')).toHaveValue('private_draft')
  expect(document.querySelector('[data-world-host]')).toBe(host); expect(f.stats.mounts).toBe(mounts); expect(f.stats.peakGames).toBe(1); expect(f.factories()).toBe(factories)
  expect(f.auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
})
it('saves a private replacement and requalifies the default shell after canonical policy withdrawal', async () => {
  const f = setup(); fireEvent.change(await screen.findByLabelText('Replacement Instagram account name'), { target: { value: 'replacement_name' } })
  fireEvent.click(screen.getByRole('button', { name: 'Replace privately' }))
  await waitFor(() => expect(f.auth.privacy.capture().policyGeneration).toBeGreaterThan(f.scope.policyGeneration))
  await screen.findByText('Configured privately · Version 4'); expect(writes(f)).toHaveLength(1)
  expect(await writes(f)[0]!.json()).toEqual({ expected_revision: 3, handle: 'replacement_name' }); expect(f.stats.peakGames).toBe(1)
})
it('locks an uncertain handle replacement and reads the pending status using its original key without another PUT', async () => {
  const f = setup(); f.service.mode('uncertain')
  fireEvent.change(await screen.findByLabelText('Replacement Instagram account name'), { target: { value: 'uncertain_name' } })
  fireEvent.click(screen.getByRole('button', { name: 'Replace privately' })); await screen.findByRole('button', { name: 'Check change status' })
  const key = writes(f)[0]!.headers.get('Idempotency-Key'); expect(key).toBeTruthy()
  fireEvent.click(screen.getByRole('link', { name: 'Requests' })); await screen.findByRole('button', { name: 'Stay with current action' }); expect(f.router.state.location.pathname).toBe('/me/social')
  fireEvent.click(screen.getByRole('button', { name: 'Stay with current action' })); f.service.mode('valid')
  fireEvent.click(screen.getByRole('button', { name: 'Check change status' }))
  await waitFor(() => expect(f.requests.filter(r => new URL(r.url).pathname.endsWith('/commands'))).toHaveLength(1))
  const status = new URL(f.requests.find(r => new URL(r.url).pathname.endsWith('/commands'))!.url)
  expect(status.searchParams.get('operation_id')).toBe('setConcertHandle'); expect(status.searchParams.get('idempotency_key')).toBe(key); expect(writes(f)).toHaveLength(1)
  expect(screen.getByLabelText('Replacement Instagram account name')).toHaveValue('uncertain_name'); expect(screen.getByRole('button', { name: 'Replace privately' })).toBeDisabled()
})
it.each(['/me/social', '/me/requests'])('missing trusted requests readiness keeps %s unavailable despite canonical Auth/C03', async path => {
  const f = setup(path, false); await screen.findByText('This section is unavailable for the current account or event.')
  expect(f.requests).toHaveLength(0)
})
it('read-only disables replacement while keeping explicit removal and metadata reads', async () => {
  const f = setup('/me/social', true, true)
  expect(await screen.findByLabelText('Replacement Instagram account name')).toBeDisabled(); expect(screen.getByRole('button', { name: 'Remove Instagram' })).toBeEnabled()
  expect(writes(f)).toHaveLength(0)
})
it('does not override a disabled existing C03 sharing capability with a ready producer', async () => {
  const f = setup('/me/social', true, false, false)
  await screen.findByText('This section is unavailable for the current account or event.'); expect(f.requests).toHaveLength(0)
})
it('starts the same lazy module when trusted readiness arrives and scrubs its draft on withdrawal', async () => {
  const f = setup('/me/social', false); await screen.findByText('This section is unavailable for the current account or event.')
  const factories = f.factories(), host = document.querySelector('[data-world-host]')
  act(() => f.ready(true)); fireEvent.change(await screen.findByLabelText('Replacement Instagram account name'), { target: { value: 'withdrawn_draft' } })
  act(() => f.ready(false)); await screen.findByText('This section is unavailable for the current account or event.')
  act(() => f.ready(true)); expect(await screen.findByLabelText('Replacement Instagram account name')).toHaveValue('')
  expect(f.factories()).toBe(factories); expect(document.querySelector('[data-world-host]')).toBe(host)
})
it('preserves the injected RequestsModule precedence and one lifetime across both declared sharing destinations', async () => {
  let created = 0, started = 0, disposed = 0
  const f = setup('/me/social', true, false, true, { create() { created++; return {
    read: () => ({ locked: false }), subscribe: () => () => {}, start() { started++ }, dispose() { disposed++ }, Panel: () => <p>Trusted custom requests adapter</p>,
  } } })
  await screen.findByText('Trusted custom requests adapter'); const count = created
  await act(() => f.router.navigate('/me/requests')); await screen.findByText('Trusted custom requests adapter')
  expect(created).toBe(count); expect(started).toBe(count); expect(f.requests).toHaveLength(0)
  act(() => f.unmount()); expect(disposed).toBe(created)
})
it.each(['policy', 'identity', 'read-only', 'dispose'] as const)('uncertain sharing command is never replayed across canonical %s withdrawal', async kind => {
  const f = setup('/me/requests'); f.service.pending(); f.service.mode('uncertain')
  fireEvent.click(await screen.findByRole('button', { name: 'Open request' })); fireEvent.click(await screen.findByRole('button', { name: 'Reject request' }))
  await screen.findByRole('button', { name: 'Check change status' }); const key = writes(f)[0]!.headers.get('Idempotency-Key')
  if (kind === 'policy') act(() => { f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'consent-revoked') })
  if (kind === 'identity') act(() => { f.auth.session.replace(f.auth.session.read().generation, null) })
  if (kind === 'read-only') act(() => f.readOnly())
  if (kind === 'dispose') act(() => f.unmount())
  await waitFor(() => expect(screen.queryByRole('button', { name: 'Check change status' })).not.toBeInTheDocument())
  if (kind === 'read-only') { await screen.findByRole('button', { name: 'Open request' }); fireEvent.click(screen.getByRole('button', { name: 'Open request' })); expect(await screen.findByRole('button', { name: 'Reject request' })).toBeDisabled() }
  expect(writes(f)).toHaveLength(1); expect(writes(f)[0]!.headers.get('Idempotency-Key')).toBe(key); expect(f.auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
})
it('retains the original uncertain command across inner Back, shell Back, links and POP, then verifies receipt without replay', async () => {
  const f = setup('/me/requests'); f.service.pending(); f.service.mode('uncertain')
  fireEvent.click(await screen.findByRole('button', { name: 'Open request' })); fireEvent.click(await screen.findByRole('button', { name: 'Reject request' }))
  await screen.findByRole('button', { name: 'Check change status' }); const sent = writes(f)[0]!, key = sent.headers.get('Idempotency-Key')
  expect(key).toBeTruthy(); expect(screen.getByRole('button', { name: 'Back to requests' })).toBeDisabled()
  fireEvent.click(screen.getByRole('link', { name: 'Social links' })); await screen.findByRole('button', { name: 'Stay with current action' }); expect(f.router.state.location.pathname).toBe('/me/requests')
  fireEvent.click(screen.getByRole('button', { name: 'Stay with current action' })); await act(() => f.router.navigate(-1)); await screen.findByRole('button', { name: 'Stay with current action' })
  fireEvent.click(screen.getByRole('button', { name: 'Stay with current action' })); fireEvent.click(screen.getByRole('button', { name: 'Back to previous panel' })); await screen.findByRole('button', { name: 'Stay with current action' })
  f.service.mode('foreign-receipt'); fireEvent.click(screen.getByRole('button', { name: 'Check change status' })); await screen.findByText('The outcome is not confirmed. Your change may have completed. Check status before submitting again.')
  expect(screen.getByRole('button', { name: 'Back to requests' })).toBeDisabled()
  f.service.mode('resolved'); fireEvent.click(screen.getByRole('button', { name: 'Check change status' })); await screen.findByText('The server confirmed this change.')
  expect(writes(f)).toHaveLength(1); expect(writes(f)[0]!.headers.get('Idempotency-Key')).toBe(key)
  expect(f.requests.filter(r => new URL(r.url).pathname.endsWith('/commands'))).toHaveLength(2)
  expect(f.requests.filter(r => new URL(r.url).pathname.endsWith('/commands')).every(r => r.url.includes(key!))).toBe(true)
  expect(screen.getByRole('button', { name: 'Back to requests' })).toBeEnabled()
})
it('canonical sensitive reveal makes one attempt on 401 and never refreshes or replays', async () => {
  const f = setup('/me/requests'); f.service.mode('denied-reveal'); fireEvent.click(await detail())
  await screen.findByText('Sign in to continue')
  expect(f.requests.filter(r => r.url.includes('/reveal'))).toHaveLength(1); expect(f.requests.filter(r => r.url.includes('/auth/refresh'))).toHaveLength(0)
  expect(screen.queryByText(`@${f.service.secret}`)).not.toBeInTheDocument()
})
it.each(['policy', 'identity', 'readiness', 'detail-back', 'route'] as const)('late reveal cannot repopulate after actual default %s transition', async kind => {
  const f = setup('/me/requests'); f.service.mode('late-reveal'); fireEvent.click(await detail())
  await waitFor(() => expect(f.requests.filter(r => r.url.includes('/reveal'))).toHaveLength(1))
  if (kind === 'policy') act(() => { f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'consent-revoked') })
  if (kind === 'identity') act(() => { f.auth.session.replace(f.auth.session.read().generation, null) })
  if (kind === 'readiness') act(() => f.ready(false))
  if (kind === 'detail-back') fireEvent.click(screen.getByRole('button', { name: 'Back to requests' }))
  if (kind === 'route') await act(() => f.router.navigate('/me/social'))
  await act(async () => { f.service.release(); await Promise.resolve() })
  expect(screen.queryByText(`@${f.service.secret}`)).not.toBeInTheDocument(); expect(f.auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
  expect(f.requests.filter(r => r.url.includes('/reveal'))).toHaveLength(1)
})
