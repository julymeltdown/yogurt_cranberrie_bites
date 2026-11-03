import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../src/app/router'
import { createProductionPorts } from '../../../src/app/ports'
import { createSessionController } from '../../../src/features/auth/model/session'
import '../../../src/shared/styles/base.css'
import '../../../src/app/shell.css'

// Test-only composition. Requests use native fetch and the actual backend, never fixture responses.
const auth = createSessionController({ eventId: import.meta.env.VITE_MCR_EVENT_ID,
  syntheticEnrollment: true })
auth.start()
const router = createAppRouter(createProductionPorts(auth))
declare global { interface Window { __MCR_REAL_EMAIL__: {
  go(path: string): Promise<void>
  snapshot(): { readiness: string; hasAccess: boolean; generation: number; userId: string | null }
  dispose(): void
} } }
window.__MCR_REAL_EMAIL__ = {
  go: async path => { await router.navigate(path) },
  snapshot: () => ({ readiness: auth.read().readiness, hasAccess: auth.session.read().accessToken !== null,
    generation: auth.session.read().generation, userId: auth.session.read().userId }),
  dispose() { router.dispose(); auth.dispose() },
}
createRoot(document.getElementById('root')!).render(<StrictMode>
  <p role="note">LOCAL REAL-SERVICE QUALIFICATION. Accounts and email use disposable local services. Eligibility and consent inputs are synthetic; production signup remains closed. World entry is not qualified.</p>
  <RouterProvider router={router} />
</StrictMode>)
