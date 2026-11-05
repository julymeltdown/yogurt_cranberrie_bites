import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../src/app/router'
import { createProductionPorts } from '../../src/app/ports'
import { createSessionController } from '../../src/features/auth/model/session'
import bootstrap from '../../../contracts/fixtures/core/bootstrap-open.json'
import '../../src/shared/styles/base.css'
import '../../src/app/shell.css'

// Controlled browser fixture only; the production entry never imports this file.
const auth = createSessionController({ eventId: bootstrap.event.event_id, timeoutMs: 1500 })
let cleanups = 0
auth.privacy.registerCleanup(() => { ++cleanups })
auth.start()
const router = createAppRouter(createProductionPorts(auth))
declare global { interface Window { __OAUTH_HANDOFF_FIXTURE__: {
  read(): { authenticated: boolean; admission: string; generation: number; cleanups: number }
  go(path: string): Promise<void>; logout(): Promise<boolean>; dispose(): void
} } }
window.__OAUTH_HANDOFF_FIXTURE__ = {
  read: () => ({ authenticated: auth.session.read().userId !== null, admission: auth.read().admission ?? 'unchecked', generation: auth.session.read().generation, cleanups }),
  go: async path => { await router.navigate(path) }, logout: auth.logout, dispose: () => { router.dispose(); auth.dispose() },
}
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router} /></StrictMode>)
