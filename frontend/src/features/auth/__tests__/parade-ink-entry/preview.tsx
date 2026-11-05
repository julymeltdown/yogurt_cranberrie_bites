import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController } from '../../model/session'
import { capabilities, eventId, isolatedCookiePolicy } from '../fixtures/data'
import { policy, accepted, attempt, admission } from '../enrollment-consent/fixtures'
import { PublicDocumentsProvider } from '../../../public/model/document-provider'
import { syntheticDocuments } from '../../../public/__tests__/fixtures/documents'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'
if (!import.meta.env.DEV) throw new Error('PI10 fixture is development-only')
const params = new URLSearchParams(location.search), mode = params.get('mode') ?? 'normal'
const path = params.get('path') ?? '/login'
const calls: string[] = []
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), timeoutMs: 1500, fetch: async input => {
  const request = input as Request, url = new URL(request.url); calls.push(url.pathname)
  const key = url.pathname.split('/').at(-1)
  if (key === 'capabilities') return Response.json(capabilities())
  if (key === 'policy') return Response.json(policy())
  if (key === 'admission') return Response.json(admission(mode === 'consent' ? 'CONSENT_REQUIRED' : mode === 'inactive' ? 'ACCOUNT_INACTIVE' : 'ERROR'))
  if (key === 'attempts') return Response.json(attempt(), { status: 201 })
  if (mode === 'pending') return new Promise<Response>((_resolve, reject) => request.signal.addEventListener('abort', () => reject(new DOMException('Aborted', 'AbortError')), { once: true }))
  if (mode === 'rate-limited') return Response.json({ code: 'RATE_LIMITED', message: 'Synthetic rate limit' }, { status: 429, headers: { 'Retry-After': '10' } })
  if (key === 'request') return Response.json({ status: 'SENT' })
  if (key === 'signup' || key === 'resend') return Response.json(accepted(), { status: 202 })
  return Response.json({ code: 'UNAVAILABLE', message: 'Synthetic unavailable' }, { status: 503 })
} })
auth.start()
if (mode === 'consent' || mode === 'inactive') auth.session.replace(auth.session.read().generation, { userId: '33333333-3333-4333-8333-333333333333', eventId, accessToken: 'PI10-synthetic-memory-only' })
history.replaceState(null, '', path)
const router = createAppRouter(createProductionPorts(auth))
Object.assign(window, { __PI10__: { go: (to: string) => router.navigate(to), calls: () => [...calls],
  state: () => ({ readiness: auth.read().readiness, hasAccess: auth.session.read().accessToken !== null }),
  dispose: () => { router.dispose(); auth.dispose() } } })
const app = <RouterProvider router={router} />
createRoot(document.getElementById('root')!).render(params.get('documents') === 'published'
  ? <PublicDocumentsProvider provider={syntheticDocuments}>{app}</PublicDocumentsProvider> : app)
