import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from './app/router'
import { createSessionController } from './features/auth/model/session'
import { canonicalUuid } from './features/auth/model/contract'
import { readLocalInputConfiguration } from './app/world-entry/configuration'
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
const ports = createProductionPorts(auth, { localInput: readLocalInputConfiguration(import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG) })
const router = createAppRouter(ports)

const application = createRoot(root)
application.render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)

import.meta.hot?.dispose(() => { application.unmount(); router.dispose(); ports.dispose?.(); auth.dispose() })
