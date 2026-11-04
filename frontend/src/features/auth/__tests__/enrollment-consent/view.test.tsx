// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { PortsProvider } from '../../../../app/access'
import type { SessionController } from '../../model/session'
import { SignUp } from '../../ui/SignUp'
import { VerifyEmail } from '../../ui/VerifyEmail'
import { ConsentAdmission } from '../../ui/ConsentAdmission'
import { fixture, defaultHandler, policy, admission, ack, email, password, attemptId, userId, type Handler } from './fixtures'
import { TransportError, parseServerError } from '../../../../shared/api/errors'
const live: { dispose(): void }[] = []
afterEach(() => { cleanup(); live.splice(0).forEach(item => item.dispose()) })
function setup(path = '/signup', handler: Handler = defaultHandler, signedIn = false) {
  const f = fixture(handler); live.push(f)
  if (signedIn) f.session.replace(0, { userId, eventId: null, accessToken: 'canonical' })
  const onAdmitted = vi.fn()
  // Exact typed feature port on a minimal test Auth host; never a production adapter.
  const auth = { enrollment: f.enrollment, session: f.session, privacy: f.privacy,
    read: () => null, subscribe: () => () => {}, checkEnrollmentAvailability: async () => {} } as unknown as SessionController
  const router = createMemoryRouter([{ path: '/signup', element: <SignUp /> }, { path: '/verify-email', element: <VerifyEmail /> },
    { path: '/consent', element: <ConsentAdmission onAdmitted={onAdmitted} /> }], { initialEntries: [path] }); live.push(router)
  const view = render(<PortsProvider ports={{ auth, session: f.session, readiness: { read: () => 'anonymous', subscribe: () => () => {} },
    bootstrap: { load: async () => { throw new Error('No world in feature qualification') } }, admin: { authorize: async () => false } }}><RouterProvider router={router} /></PortsProvider>)
  return { ...f, router, view, onAdmitted }
}
it('loads actual wire document titles and exact explicit acknowledgments, no fixture booleans', async () => {
  const f = setup(); expect(f.requests).toHaveLength(0)
  expect(screen.queryByRole('checkbox')).toBeNull(); expect(screen.getByRole('button', { name: 'Create account' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Load current agreements' }))
  expect(await screen.findByRole('link', { name: policy().documents[0]!.title })).toHaveAttribute('href', policy().documents[0]!.url)
  expect(screen.queryByText(/eligibility requirements in this test scenario/)).toBeNull()
  fireEvent.click(screen.getByRole('checkbox')); expect(screen.getByRole('button', { name: 'Create account' })).toBeEnabled()
  fireEvent.change(screen.getByLabelText('Email'), { target: { value: email } }); fireEvent.change(screen.getByLabelText('Password'), { target: { value: password } })
  fireEvent.submit(screen.getByRole('form')); fireEvent.submit(screen.getByRole('form'))
  await screen.findByText(/Account creation and email delivery are not confirmed/)
  expect(f.requests.filter(request => request.path.endsWith('/signup'))).toHaveLength(1)
  expect(f.requests.at(-1)!.body).toMatchObject({ json: { acknowledgementIds: ack, email, password } })
  expect(screen.queryByLabelText('Password')).toBeNull()
})
it('captures only attempt+code and scrubs URL before requests; never asks for email', async () => {
  const f = setup(`/verify-email?attemptId=${attemptId}&code=123456`)
  await waitFor(() => expect(f.router.state.location.search).toBe(''))
  expect(screen.getByLabelText('Signup attempt')).toHaveValue(attemptId)
  expect(screen.getByLabelText('Verification code')).toHaveValue('123456'); expect(screen.queryByLabelText('Email')).toBeNull()
  expect(f.requests).toHaveLength(0); fireEvent.submit(screen.getByRole('form'))
  expect(screen.getByLabelText('Verification code')).toHaveValue('')
  await screen.findByText('Email verified. Sign in to continue.')
  expect(f.requests[0]!.body).toEqual({ json: { attemptId, code: '123456' } })
})
it.each([`?email=private@example.invalid&attemptId=${attemptId}&code=123456`, `?attemptId=${attemptId}&code=123456&code=654321`, `?attemptId=${attemptId}&code=123456#secret`])('rejects polluted/duplicate prefills and strips URL %s', async query => {
  const f = setup('/verify-email' + query)
  await waitFor(() => expect(f.router.state.location.search + f.router.state.location.hash).toBe(''))
  expect(screen.getByLabelText('Verification code')).toHaveValue(''); expect(screen.getByLabelText('Signup attempt')).toHaveValue(''); expect(f.requests).toHaveLength(0)
})
it('never continues World on local acceptance or a later nonadmitted server decision', async () => {
  const f = setup('/consent', request => request.path.endsWith('/admission') ? { status: 200, data: admission('CONSENT_REQUIRED') } : defaultHandler(request), true)
  expect(f.requests).toHaveLength(0); fireEvent.click(screen.getByRole('button', { name: 'Load current agreements' })); await screen.findByRole('checkbox')
  fireEvent.click(screen.getByRole('checkbox')); fireEvent.click(screen.getByRole('button', { name: 'Agree and continue' }))
  await screen.findByText('Read and accept the current published agreements to continue.'); expect(f.onAdmitted).not.toHaveBeenCalled()
  expect(f.requests.at(-1)!.path).toBe('/api/auth/enrollment/admission')
})
it('calls Root continuation only after protected consent AND fresh admitted read', async () => {
  const f = setup('/consent', defaultHandler, true)
  fireEvent.click(screen.getByRole('button', { name: 'Load current agreements' })); await screen.findByRole('checkbox')
  fireEvent.click(screen.getByRole('checkbox')); fireEvent.click(screen.getByRole('button', { name: 'Agree and continue' }))
  await waitFor(() => expect(f.onAdmitted).toHaveBeenCalledTimes(1)); expect(f.requests.slice(-2).map(request => request.path)).toEqual(['/api/auth/enrollment/consent', '/api/auth/enrollment/admission'])
})
it('shows honest uncertainty with explicit original retry and clears password immediately', async () => {
  const f = setup('/signup', request => { if (request.path.endsWith('/signup')) throw new TransportError('network', { uncertain: true }); return defaultHandler(request) })
  fireEvent.click(screen.getByRole('button', { name: 'Load current agreements' })); await screen.findByRole('checkbox'); fireEvent.click(screen.getByRole('checkbox'))
  fireEvent.change(screen.getByLabelText('Email'), { target: { value: email } }); fireEvent.change(screen.getByLabelText('Password'), { target: { value: password } })
  fireEvent.submit(screen.getByRole('form')); await screen.findByRole('button', { name: 'Retry original request' })
  expect(screen.getByLabelText('Password')).toHaveValue(''); expect(screen.getByRole('button', { name: 'Create account' })).toBeDisabled()
  expect(f.requests.filter(request => request.path.endsWith('/signup'))).toHaveLength(1)
})

it('offers explicit fresh signup after server attempt expiry; never automatically starts another attempt', async () => {
  const f = setup(`/verify-email?attemptId=${attemptId}&code=123456`, () => {
    throw new TransportError('http', { status: 410, server: parseServerError({ code: 'EXPIRED', message: 'private', details: { reason: 'ATTEMPT_EXPIRED' } }, 'auth', 410) })
  })
  await waitFor(() => expect(f.router.state.location.search).toBe(''))
  fireEvent.submit(screen.getByRole('form'))
  const restart = await screen.findByRole('button', { name: 'Start again' }); expect(f.requests).toHaveLength(1)
  fireEvent.click(restart); await screen.findByRole('heading', { name: 'Create account' }); expect(f.requests).toHaveLength(1)
})
