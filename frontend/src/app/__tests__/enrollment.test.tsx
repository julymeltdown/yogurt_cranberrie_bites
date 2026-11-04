// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../router'
import { createProductionPorts } from '../ports'
import { createSessionController, type SessionController } from '../../features/auth/model/session'
import { capabilities, deferred, eventId, isolatedCookiePolicy } from '../../features/auth/__tests__/fixtures/data'
import { accepted, admission, attempt, attemptId, policy, email, password, ack } from '../../features/auth/__tests__/enrollment-consent/fixtures'
const code = '123456', routers: ReturnType<typeof createMemoryRouter>[] = [], controllers: SessionController[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(r => r.dispose()); controllers.splice(0).forEach(c => c.dispose()); vi.useRealTimers() })
const normal = (r: Request): Response => {
  const key = new URL(r.url).pathname.split('/').at(-1)
  return Response.json(key === 'policy' ? policy() : key === 'attempts' ? attempt() : key === 'admission' || key === 'consent' ? admission()
    : key === 'verify' ? { status: 'VERIFIED', attemptId, serverTime: new Date().toISOString() } : accepted(), { status: key === 'attempts' ? 201 : key === 'signup' || key === 'resend' ? 202 : 200 })
}
async function setup(path = '/signup', handler: (r: Request) => Response | Promise<Response> = normal, event: string | null = eventId) {
  const requests: Request[] = []
  const fetch = vi.fn<typeof globalThis.fetch>(async input => { const r = input as Request
    if (r.url.endsWith('/capabilities')) return Response.json(capabilities())
    requests.push(r.clone()); return handler(r)
  })
  const auth = createSessionController({ eventId: event, fetch, cookiePolicy: isolatedCookiePolicy(), timeoutMs: 500 }); controllers.push(auth)
  auth.start(); if (event === eventId) await vi.waitFor(() => expect(auth.read().capabilities).toBe('ready'))
  const router = createMemoryRouter(createAppRoutes(createProductionPorts(auth)), { initialEntries: [path] }); routers.push(router)
  render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  await screen.findByRole('heading', { name: path.startsWith('/signup') ? 'Create account' : 'Verify email' })
  await waitFor(() => expect(router.state.location.search + router.state.location.hash).toBe(''))
  return { auth, router, requests, fetch }
}
async function fillSignup() {
  fireEvent.click(screen.getByRole('button', { name: 'Load current agreements' })); await screen.findByRole('checkbox')
  fireEvent.change(screen.getByLabelText('Email'), { target: { value: email } }); fireEvent.change(screen.getByLabelText('Password'), { target: { value: password } })
  fireEvent.click(screen.getByRole('checkbox'))
}
function fillVerify() {
  fireEvent.change(screen.getByLabelText('Signup attempt'), { target: { value: attemptId } })
  fireEvent.change(screen.getByLabelText('Verification code'), { target: { value: code } })
}
it('requires an explicit published policy read and never manufactures consent from a qualification flag', async () => {
  const { auth, requests } = await setup()
  expect(screen.getByRole('button', { name: 'Create account' })).toBeDisabled(); expect(screen.queryByRole('checkbox')).toBeNull()
  expect(requests).toHaveLength(0); expect(auth.session.read().userId).toBeNull(); expect(document.querySelector('canvas,[data-world-host]')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Load current agreements' }))
  expect(await screen.findByRole('link', { name: policy().documents[0]!.title })).toHaveAttribute('href', policy().documents[0]!.url)
})
it('uses policy/attempt/acknowledgments, guards duplicate submit, clears secrets and keeps neutral signup feedback', async () => {
  const late = deferred<Response>(), { auth, requests } = await setup('/signup', r => r.url.endsWith('/signup') ? late.promise : normal(r))
  await fillSignup(); const form = screen.getByRole('form', { name: 'Create account' })
  fireEvent.submit(form); fireEvent.submit(form)
  expect(screen.getByLabelText('Password')).toHaveValue('')
  await waitFor(() => expect(requests.filter(r => r.url.endsWith('/signup'))).toHaveLength(1))
  const body = await requests.at(-1)!.json(); expect(body).toMatchObject({ attemptId, policyRevision: policy().revision, acknowledgementIds: ack, email, password })
  expect(body.idempotencyKey).toMatch(/^[a-f0-9-]{36}$/)
  await act(async () => late.resolve(normal(requests.at(-1)!)))
  const outcome = await screen.findByText(/Account creation and email delivery are not confirmed/)
  expect(outcome.closest('[role="status"]')).toHaveFocus()
  expect(screen.getByRole('link', { name: 'Continue to email verification' })).toHaveAttribute('href', '/verify-email')
  expect(document.body.innerHTML).not.toContain(password); expect(auth.session.read().accessToken).toBeNull()
})
it('verifies only opaque attempt and code, scrubs link before dispatch and never installs an identity', async () => {
  const { auth, requests } = await setup(`/verify-email?attemptId=${attemptId}&code=${code}`)
  expect(requests).toHaveLength(0); expect(screen.queryByLabelText('Email')).toBeNull()
  fireEvent.submit(screen.getByRole('form', { name: 'Verify email' }))
  expect(await screen.findByText('Email verified. Sign in to continue.')).toBeVisible()
  expect(await requests[0]!.json()).toEqual({ attemptId, code }); expect(auth.session.read().accessToken).toBeNull()
  expect(document.body.innerHTML).not.toContain(code)
})
it('keeps shadow/duplicate signup generic and excludes raw server details', async () => {
  const { requests } = await setup('/signup', r => r.url.endsWith('/signup') ? Response.json({ code: 'CONFLICT', message: 'PRIVATE_EXISTS', details: { reason: 'IDEMPOTENCY_CONFLICT' } }, { status: 409 }) : normal(r))
  await fillSignup(); fireEvent.submit(screen.getByRole('form'))
  await screen.findByRole('button', { name: 'Start again' }); expect(document.body.textContent).not.toContain('PRIVATE_EXISTS')
  expect(requests.filter(r => r.url.endsWith('/signup'))).toHaveLength(1)
})
it('treats unusable challenge generically and resends with attempt/idempotency rather than email', async () => {
  const { requests } = await setup('/verify-email', r => r.url.endsWith('/verify') ? Response.json({ code: 'BAD', message: 'PRIVATE_EXPIRED', details: { reason: 'VERIFICATION_FAILED' } }, { status: 422 }) : normal(r))
  fillVerify(); fireEvent.submit(screen.getByRole('form'))
  await screen.findByText(/It may be invalid, expired or already used/); expect(screen.getByLabelText('Verification code')).toHaveValue('')
  fireEvent.click(screen.getByRole('button', { name: 'Request another code' }))
  await screen.findByText(/Account creation and email delivery are not confirmed/)
  expect(await requests[1]!.json()).toEqual({ attemptId, idempotencyKey: expect.any(String) })
  expect(screen.getByRole('button', { name: 'Request another code' })).toBeDisabled()
})
it('honors server Retry-After without an automatic resend at expiry', async () => {
  const { requests } = await setup('/verify-email', () => Response.json({ code: 'RATE', message: 'PRIVATE_RATE', details: { reason: 'RATE_LIMITED' } }, { status: 429, headers: { 'Retry-After': '1' } }))
  fillVerify(); fireEvent.click(screen.getByRole('button', { name: 'Request another code' }))
  await screen.findByText(/Server requested a wait/); expect(screen.getByRole('button', { name: 'Verify email' })).toBeDisabled()
  await waitFor(() => expect(screen.getByRole('button', { name: 'Request another code' })).toBeEnabled(), { timeout: 2000 })
  expect(requests).toHaveLength(1)
})
it('retains an uncertain resend only for explicit original retry', async () => {
  const { requests } = await setup('/verify-email', () => { throw new TypeError('PRIVATE_NETWORK') })
  fillVerify(); fireEvent.click(screen.getByRole('button', { name: 'Request another code' }))
  await screen.findByRole('button', { name: 'Retry original request' })
  expect(requests).toHaveLength(1); expect(screen.queryByText(/Email verified/)).toBeNull(); expect(document.body.textContent).not.toContain('PRIVATE_NETWORK')
})
it('cancels pending verification on route exit and ignores its late completion', async () => {
  const late = deferred<Response>(), { router, requests } = await setup('/verify-email', () => late.promise)
  fillVerify(); fireEvent.submit(screen.getByRole('form')); await waitFor(() => expect(requests).toHaveLength(1))
  await act(async () => { await router.navigate('/signup') }); expect(requests[0]!.signal.aborted).toBe(true)
  await act(async () => late.resolve(normal(requests[0]!)))
  expect(screen.queryByText(/Email verified/)).toBeNull(); expect(screen.getByLabelText('Password')).toHaveValue('')
})
it.each(['/signup', '/verify-email'])('rejects polluted query/hash prefills and clears private fields on %s', async path => {
  const { router, requests } = await setup(`${path}?email=PRIVATE_EMAIL&attemptId=${attemptId}&code=${code}#secret`)
  expect(router.state.historyAction).toBe('REPLACE'); expect(router.state.location.state).toBeNull()
  expect(document.body.innerHTML).not.toMatch(/PRIVATE_EMAIL|123456/); expect(requests).toHaveLength(0)
})
it.each(['/signup', '/verify-email'])('clears unsent secrets after privacy invalidation on %s', async path => {
  const { auth } = await setup(path)
  if (path === '/signup') await fillSignup(); else fillVerify()
  await act(async () => { auth.privacy.changePolicy(auth.privacy.capture(), 'consent-revoked') })
  expect(screen.getByLabelText(path === '/signup' ? 'Password' : 'Verification code')).toHaveValue('')
  expect(screen.getByText(/Your session changed/)).toBeVisible()
})
it.each([null, 'malformed-event'])('allows explicit enrollment availability without a configured event %j', async event => {
  const { auth, requests, fetch } = await setup('/verify-email', normal, event)
  expect(fetch).not.toHaveBeenCalled(); fireEvent.click(screen.getByRole('button', { name: 'Check email availability' }))
  await waitFor(() => expect(screen.getByLabelText('Signup attempt')).toBeEnabled())
  fillVerify(); fireEvent.submit(screen.getByRole('form')); await screen.findByText('Email verified. Sign in to continue.')
  expect(requests).toHaveLength(1); expect(auth.session.read().userId).toBeNull(); expect(auth.read().readiness).toBe('unavailable')
})
