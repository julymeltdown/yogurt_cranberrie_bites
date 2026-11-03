import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController } from '../../model/session'
import { eventId } from './data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const params = new URLSearchParams(location.search), callback = params.get('screen') === 'callback'
const hash = location.hash
history.replaceState(null, '', callback ? '/auth/callback?error=access_denied&state=SYNTHETIC_FE11_STATE&returnTo=%2Ffeed'
  : `/reset-password?returnTo=%2Ffeed${hash}`)
const nativeFetch = globalThis.fetch.bind(globalThis), dispatches = { request: 0, confirm: 0 }
const observedFetch: typeof fetch = (input, init) => {
  const url = input instanceof Request ? input.url : String(input)
  if (url.endsWith('/password/reset/request')) dispatches.request++
  if (url.endsWith('/password/reset/confirm')) dispatches.confirm++
  return nativeFetch(input, init)
}
const auth = createSessionController({ eventId, timeoutMs: 700, fetch: observedFetch })
auth.start()
const router = createAppRouter(createProductionPorts(auth))
declare global { interface Window { __FE11_TEST__: {
  hasAccess(): boolean; generation(): number
  login(): Promise<boolean>; logout(): Promise<boolean>
  go(path: string): Promise<void>; dispose(): void
  dispatches(): { request: number; confirm: number }
} } }
window.__FE11_TEST__ = {
  hasAccess: () => auth.session.read().accessToken !== null, generation: () => auth.session.read().generation,
  login: () => auth.login('synthetic@example.com', 'SYNTHETIC_FE11_LOGIN_PASSWORD'), logout: auth.logout,
  dispatches: () => ({ ...dispatches }),
  go: async path => { await router.navigate(path) }, dispose: () => { router.dispose(); auth.dispose() },
}
createRoot(document.getElementById('root')!).render(<StrictMode>
  <p role="note">SYNTHETIC_FE11_RECOVERY — UI qualification only. No real account, mail delivery or OAuth provider.</p>
  <RouterProvider router={router} />
</StrictMode>)
