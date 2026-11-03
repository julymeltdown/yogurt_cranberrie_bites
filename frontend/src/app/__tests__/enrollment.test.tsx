// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createAppRoutes } from '../router'
import { createProductionPorts } from '../ports'
import { createSessionController, type SessionController } from '../../features/auth/model/session'
import { capabilities, deferred, eventId, isolatedCookiePolicy, userId } from '../../features/auth/__tests__/fixtures/data'
const email = 'synthetic@example.com', password = '  secret enrollment password  ', code = '123456'
const routers: ReturnType<typeof createMemoryRouter>[] = [], controllers: SessionController[] = []
afterEach(() => { cleanup(); routers.splice(0).forEach(r => r.dispose()); controllers.splice(0).forEach(c => c.dispose()); vi.useRealTimers() })
async function setup(path = '/signup', handler: (r: Request) => Response | Promise<Response> = r => Response.json(r.url.endsWith('/resend') ? { status: 'ACCEPTED' } : { userId, status: r.url.endsWith('/signup') ? 'PENDING_VERIFICATION' : 'VERIFIED' }, { status: r.url.endsWith('/resend') ? 202 : 200 }), syntheticEnrollment = true, event: string | null = eventId) {
  const requests: Request[] = []
  const fetch = vi.fn<typeof globalThis.fetch>(async input => { const r = input as Request
    if (r.url.endsWith('/capabilities')) return Response.json(capabilities())
    requests.push(r.clone()); return handler(r)
  })
  const auth = createSessionController({ eventId: event, fetch, cookiePolicy: isolatedCookiePolicy(), syntheticEnrollment, timeoutMs: 500 }); controllers.push(auth)
  auth.start(); if (event && event !== 'malformed-event') await vi.waitFor(() => expect(auth.read().capabilities).toBe('ready'))
  const router = createMemoryRouter(createAppRoutes(createProductionPorts(auth)), { initialEntries: [path] }); routers.push(router)
  const view = render(<StrictMode><RouterProvider router={router} /></StrictMode>)
  await screen.findByRole('heading', { name: path.startsWith('/signup') ? 'Create account' : 'Verify email' })
  return { auth, router, requests, fetch, ...view }
}
function fillSignup() {
  fireEvent.change(screen.getByLabelText('Email'), { target: { value: email } }); fireEvent.change(screen.getByLabelText('Password'), { target: { value: password } })
  for (const checkbox of screen.getAllByRole('checkbox')) fireEvent.click(checkbox)
}
function fillVerify() {
  fireEvent.change(screen.getByLabelText('Email'), { target: { value: email } }); fireEvent.change(screen.getByLabelText('Verification code'), { target: { value: code } })
}
it('keeps production signup disabled without inventing a policy or persisted consent', async () => {
  const { fetch } = await setup('/signup', undefined, false, null)
  expect(screen.getByRole('heading')).toHaveFocus(); expect(screen.getByText(/Account creation is not open yet/)).toBeVisible()
  expect(screen.queryByRole('checkbox')).toBeNull(); expect(screen.getByRole('button', { name: 'Create account' })).toBeDisabled()
  expect(fetch).not.toHaveBeenCalled(); expect(document.querySelector('canvas,[data-world-host]')).toBeNull()
})
it('marks synthetic acknowledgements clearly, guards double submit, clears password, focuses outcome, and links without email', async () => {
  const late = deferred<Response>(), { requests, auth } = await setup('/signup', () => late.promise)
  expect(screen.getByText(/They are not recorded as consent/)).toBeVisible(); expect(screen.getByRole('button', { name: 'Create account' })).toBeDisabled()
  fillSignup(); const form = screen.getByRole('form', { name: 'Create account' })
  fireEvent.submit(form); fireEvent.submit(form)
  expect(screen.getByLabelText('Password')).toHaveValue(''); expect(screen.getByRole('button', { name: 'Create account' })).toBeDisabled()
  await waitFor(() => expect(requests).toHaveLength(1))
  await act(async () => late.resolve(Response.json({ userId, status: 'PENDING_VERIFICATION' })))
  expect(await screen.findByText(/Account created. Verify your email/)).toBeVisible()
  expect(screen.getByText(/Account created/).closest('[role="status"]')).toHaveFocus()
  expect(screen.getByRole('link', { name: 'Continue to email verification' })).toHaveAttribute('href', '/verify-email')
  expect(document.body.innerHTML).not.toContain(password); expect(auth.session.read().accessToken).toBeNull()
})
it('separates existing-account verification from unavailable signup policy and never signs in', async () => {
  const { auth, requests } = await setup('/verify-email', undefined, false)
  fillVerify(); fireEvent.submit(screen.getByRole('form', { name: 'Verify email' }))
  expect(await screen.findByText('Email verified. Sign in to continue.')).toBeVisible()
  expect(await requests[0]!.json()).toEqual({ email, code }); expect(auth.session.read().accessToken).toBeNull()
  expect(document.body.innerHTML).not.toContain(code)
})
it('keeps duplicate signup feedback neutral and excludes raw server data', async () => {
  const { requests } = await setup('/signup', () => Response.json({ code: 'PRIVATE', message: 'RAW_EMAIL_EXISTS' }, { status: 409 }))
  fillSignup(); fireEvent.submit(screen.getByRole('form', { name: 'Create account' }))
  await screen.findByText(/We could not create this account/)
  expect(document.body.textContent).not.toContain('RAW_EMAIL_EXISTS'); expect(requests).toHaveLength(1)
})
it('handles expired code and opaque resend admission without inventing a timer or delivery success', async () => {
  const { requests } = await setup('/verify-email', r => r.url.endsWith('/resend') ? Response.json({ status: 'ACCEPTED' }, { status: 202 }) : Response.json({ code: 'BAD', message: 'PRIVATE_EXPIRED_CODE' }, { status: 400 }))
  fillVerify(); fireEvent.submit(screen.getByRole('form', { name: 'Verify email' }))
  await screen.findByText(/It may be invalid, expired or already used/); expect(screen.getByLabelText('Verification code')).toHaveValue('')
  fireEvent.click(screen.getByRole('button', { name: 'Request another code' }))
  await screen.findByText(/Account details and email delivery are not confirmed here/)
  expect(requests).toHaveLength(2); expect(await requests[1]!.json()).toEqual({ email })
  expect(screen.queryByText(/Try again in/)).toBeNull(); expect(document.body.textContent).not.toContain('PRIVATE_EXPIRED_CODE')
})
it('shows a server-only cooldown that disables verify/resend then becomes actionable without replay', async () => {
  const { requests } = await setup('/verify-email', () => Response.json({ code: 'RATE', message: 'RAW_RATE' }, { status: 429, headers: { 'Retry-After': '1' } }))
  fillVerify(); fireEvent.click(screen.getByRole('button', { name: 'Request another code' }))
  await screen.findByText(/Server requested a wait/)
  expect(screen.getByRole('button', { name: 'Request another code' })).toBeDisabled(); expect(screen.getByRole('button', { name: 'Verify email' })).toBeDisabled()
  await waitFor(() => expect(screen.getByRole('button', { name: 'Request another code' })).toBeEnabled(), { timeout: 2000 })
  expect(requests).toHaveLength(1)
})
it('keeps resend uncertainty neutral about account existence and mail delivery', async () => {
  await setup('/verify-email', () => { throw new TypeError('PRIVATE_NETWORK_ERROR') }); fillVerify()
  fireEvent.click(screen.getByRole('button', { name: 'Request another code' }))
  await screen.findByText(/result and email delivery are not confirmed/)
  expect(screen.queryByText(/Email verified/)).toBeNull(); expect(screen.getByLabelText('Verification code')).toHaveValue('')
})
it.each(['signup', 'verify'] as const)('fences %s feedback and clears drafts after account replacement', async mode => {
  const late = deferred<Response>(), { auth, requests } = await setup(mode === 'signup' ? '/signup' : '/verify-email', () => late.promise)
  if (mode === 'signup') fillSignup(); else fillVerify()
  fireEvent.submit(screen.getByRole('form', { name: mode === 'signup' ? 'Create account' : 'Verify email' }))
  await waitFor(() => expect(requests).toHaveLength(1))
  await act(async () => { auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'other-account' }) })
  expect(screen.getByText(/Your session changed/)).toBeVisible(); expect(screen.getByLabelText('Email')).toHaveValue('')
  await act(async () => late.resolve(Response.json({ userId, status: mode === 'signup' ? 'PENDING_VERIFICATION' : 'VERIFIED' })))
  expect(screen.queryByText(/Account created|Email verified/)).toBeNull()
})
it.each(['/verify-email', '/signup'])('clears unsent sensitive fields on fresh same-route navigation %s', async path => {
  const { router, requests } = await setup(path)
  if (path === '/signup') fillSignup(); else fillVerify()
  await act(async () => { await router.navigate(path) })
  expect(screen.getByLabelText('Email')).toHaveValue('')
  expect(screen.getByLabelText(path === '/signup' ? 'Password' : 'Verification code')).toHaveValue('')
  expect(requests).toHaveLength(0)
})
it('cancels pending verify on route exit and excludes late completion from a fresh route', async () => {
  const late = deferred<Response>(), { router, requests } = await setup('/verify-email', () => late.promise)
  fillVerify(); fireEvent.submit(screen.getByRole('form', { name: 'Verify email' })); await waitFor(() => expect(requests).toHaveLength(1))
  await act(async () => { await router.navigate('/signup') }); expect(requests[0]!.signal.aborted).toBe(true)
  await act(async () => late.resolve(Response.json({ userId, status: 'VERIFIED' })))
  expect(screen.queryByText(/Email verified/)).toBeNull(); expect(screen.getByLabelText('Password')).toHaveValue('')
})
it.each(['/signup', '/verify-email'])('scrubs untrusted query/hash/history state before rendering %s form', async path => {
  const { router, requests } = await setup(`${path}?email=SECRET_URL_EMAIL&password=SECRET_URL_PASSWORD#code=654321`)
  await waitFor(() => expect(router.state.location.search + router.state.location.hash).toBe(''))
  expect(router.state.historyAction).toBe('REPLACE'); expect(router.state.location.state).toBeNull()
  expect(document.body.innerHTML).not.toMatch(/SECRET_URL_EMAIL|SECRET_URL_PASSWORD|654321/)
  expect(screen.getByLabelText('Email')).toHaveValue(''); expect(requests).toHaveLength(0)
})

it.each(['/signup', '/verify-email'])('clears unsent drafts after privacy invalidation on %s', async path => {
  const { auth, requests } = await setup(path)
  if (path === '/signup') fillSignup(); else fillVerify()
  await act(async () => { auth.privacy.changePolicy(auth.privacy.capture(), 'consent-revoked') })
  expect(screen.getByLabelText('Email')).toHaveValue('')
  expect(screen.getByLabelText(path === '/signup' ? 'Password' : 'Verification code')).toHaveValue('')
  if (path === '/signup') for (const box of screen.getAllByRole('checkbox')) expect(box).not.toBeChecked()
  expect(screen.getByText(/Your session changed/)).toBeVisible(); expect(requests).toHaveLength(0)
})

it.each([null, 'malformed-event'])('offers explicit existing-account email actions without event %j', async event => {
  const { auth, fetch, requests } = await setup('/verify-email', undefined, false, event)
  expect(fetch).not.toHaveBeenCalled(); expect(screen.getByRole('button', { name: 'Verify email' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Check email availability' }))
  await waitFor(() => expect(screen.getByLabelText('Email')).toBeEnabled())
  fillVerify(); fireEvent.click(screen.getByRole('button', { name: 'Request another code' }))
  await screen.findByText(/Account details and email delivery are not confirmed here/)
  fireEvent.change(screen.getByLabelText('Verification code'), { target: { value: code } })
  fireEvent.submit(screen.getByRole('form', { name: 'Verify email' }))
  await screen.findByText('Email verified. Sign in to continue.')
  expect(requests).toHaveLength(2); expect(fetch).toHaveBeenCalledTimes(3)
  expect(auth.read()).toMatchObject({ readiness: 'unavailable', emailEnabled: false })
  expect(auth.session.read().accessToken).toBeNull(); expect(document.querySelector('canvas,[data-world-host]')).toBeNull()
})
it('stops cooldown polling at expiry and clears active polling on route exit', async () => {
  const { router, requests } = await setup('/verify-email', () => Response.json({ code: 'RATE', message: 'RAW' }, { status: 429, headers: { 'Retry-After': '1' } }))
  vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] })
  fillVerify()
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Request another code' })) })
  expect(screen.getByText(/Server requested a wait/)).toBeVisible(); expect(vi.getTimerCount()).toBe(1)
  await act(async () => { await vi.advanceTimersByTimeAsync(1250) })
  expect(screen.getByRole('button', { name: 'Request another code' })).toBeEnabled(); expect(vi.getTimerCount()).toBe(0)
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) })
  expect(vi.getTimerCount()).toBe(0); expect(requests).toHaveLength(1)
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Request another code' })) })
  expect(vi.getTimerCount()).toBe(1)
  await act(async () => { await router.navigate('/legal/privacy') })
  expect(vi.getTimerCount()).toBe(0)
})
