import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController } from '../../model/session'
import { eventId } from './data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'
// Deterministic test server only; no policy is installed by this fixture.
const params = new URLSearchParams(location.search), verify = params.get('screen') === 'verify'
const query = new URLSearchParams()
for (const key of ['attemptId', 'code']) if (params.has(key)) query.set(key, params.get(key)!)
history.replaceState(null, '', verify ? `/verify-email${query.size ? `?${query}` : ''}` : '/signup')
const counts = { policy: 0, attempts: 0, signup: 0, verify: 0, resend: 0 }, nativeFetch = globalThis.fetch.bind(globalThis)
const observedFetch: typeof fetch = (input, init) => {
  const url = input instanceof Request ? input.url : String(input)
  for (const key of Object.keys(counts) as (keyof typeof counts)[]) if (url.endsWith(`/enrollment/${key}`)) counts[key]++
  return nativeFetch(input, init)
}
const auth = createSessionController({ eventId, timeoutMs: 800, fetch: observedFetch })
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
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router} /></StrictMode>)
