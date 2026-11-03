import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from './app/router'
import { createSessionController } from './features/auth/model/session'
import { canonicalUuid } from './features/auth/model/contract'
import { createProductionPorts } from './app/ports'
import './shared/styles/base.css'
import './app/shell.css'

const root = document.getElementById('root')

if (root === null) {
  throw new Error('MCR Concert World requires a root element.')
}

const configuredEvent: unknown = import.meta.env.VITE_MCR_EVENT_ID
const auth = createSessionController({ eventId: canonicalUuid(configuredEvent) ? configuredEvent : null })
auth.start()
const router = createAppRouter(createProductionPorts(auth))

createRoot(root).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)
