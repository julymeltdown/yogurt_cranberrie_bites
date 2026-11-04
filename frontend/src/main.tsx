import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from './app/router'
import { createSessionController } from './features/auth/model/session'
import { canonicalUuid } from './features/auth/model/contract'
import { readLocalInputConfiguration } from './app/world-entry/configuration'
import { createProductionPorts } from './app/ports'
import { createDefaultConnectionStatus } from './app/errors/default-entry'
import { readMuralProductionConfiguration, validateMuralProductionConfiguration } from './app/world-entry/mural-production/configuration'
import muralPlacement from './app/world-entry/mural-production/placement.json'
import './shared/styles/base.css'
import './app/shell.css'

const root = document.getElementById('root')

if (root === null) {
  throw new Error('MCR Concert World requires a root element.')
}

const configuredEvent: unknown = import.meta.env.VITE_MCR_EVENT_ID
const eventId = canonicalUuid(configuredEvent) ? configuredEvent : null
// BE03 browser rotation is qualified; the controller owns single-flight recovery and safe read replay.
const auth = createSessionController({ eventId, qualifiedRefresh: true })
auth.start()
const productionPorts = createProductionPorts(auth, { localInput: readLocalInputConfiguration(import.meta.env.VITE_MCR_LOCAL_INPUT_CONFIG), mural: import.meta.env.VITE_MCR_MURAL_CONFIG === undefined ? validateMuralProductionConfiguration(muralPlacement) : readMuralProductionConfiguration(import.meta.env.VITE_MCR_MURAL_CONFIG) }, undefined, eventId)
const connectionStatus = createDefaultConnectionStatus({ auth, world: productionPorts.worldAccess, browser: window, reload: () => location.reload() })
connectionStatus.start()
const ports = { ...productionPorts, connectionStatus }
const router = createAppRouter(ports)

const application = createRoot(root)
application.render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)

import.meta.hot?.dispose(() => { connectionStatus.dispose(); application.unmount(); router.dispose(); ports.dispose?.(); auth.dispose() })
