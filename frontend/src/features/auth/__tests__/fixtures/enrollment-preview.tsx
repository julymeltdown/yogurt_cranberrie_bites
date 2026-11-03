import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController } from '../../model/session'
import { eventId } from './data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'
const params = new URLSearchParams(location.search), verify = params.get('screen') === 'verify'
const event = params.get('event'), configuredEvent = event === 'malformed' ? 'malformed-event' : event === 'none' ? null : eventId
history.replaceState(null, '', verify ? '/verify-email' : '/signup')
const counts = { signup: 0, verify: 0, resend: 0 }, nativeFetch = globalThis.fetch.bind(globalThis)
const observedFetch: typeof fetch = (input, init) => {
  const url = input instanceof Request ? input.url : String(input)
  for (const key of Object.keys(counts) as (keyof typeof counts)[]) if (url.endsWith(`/email/${key}`)) counts[key]++
  return nativeFetch(input, init)
}
const auth = createSessionController({ eventId: configuredEvent, syntheticEnrollment: !event, timeoutMs: 800, fetch: observedFetch })
auth.start()
const router = createAppRouter(createProductionPorts(auth))
declare global { interface Window { __FE10_TEST__: {
  go(path: string): Promise<void>; clear(): void; hasAccess(): boolean
  dispatches(): typeof counts; dispose(): void
} } }
window.__FE10_TEST__ = {
  go: async path => { await router.navigate(path) }, clear: () => { auth.session.replace(auth.session.read().generation, null) },
  hasAccess: () => auth.session.read().accessToken !== null, dispatches: () => ({ ...counts }),
  dispose: () => { router.dispose(); auth.dispose() },
}
createRoot(document.getElementById('root')!).render(<StrictMode>
  <p role="note">SYNTHETIC_FE10 — UI qualification only. No real eligibility policy, recorded consent, account or mail delivery.</p>
  <RouterProvider router={router} />
</StrictMode>)
