import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController } from '../../model/session'
import { eventId } from '../fixtures/data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const auth = createSessionController({ eventId, timeoutMs: 8000 })
auth.start()
history.replaceState(null, '', '/login?returnTo=%2Ffeed')
const router = createAppRouter(createProductionPorts(auth))
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router} /></StrictMode>)
