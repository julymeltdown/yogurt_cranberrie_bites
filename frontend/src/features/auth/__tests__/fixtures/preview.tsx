import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { createAppRouter } from '../../../../app/router'
import { createProductionPorts } from '../../../../app/ports'
import { createSessionController } from '../../model/session'
import { eventId } from './data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const qualifiedRefresh = new URLSearchParams(location.search).has('qualified')
const auth = createSessionController({ eventId, qualifiedRefresh, timeoutMs: 800 })
auth.start()
const counters = { mounts: 0, disposals: 0, live: 0, peak: 0 }
const ports = createProductionPorts(auth)
const router = createAppRouter({ ...ports, world: {
  mount(host, context) {
    counters.mounts++; counters.live++; counters.peak = Math.max(counters.peak, counters.live)
    const content = document.createElement('p'); content.textContent = 'FE09 validated mock world'; host.append(content)
    let closed = false
    const dispose = () => { if (closed) return; closed = true; counters.disposals++; counters.live--; content.remove() }
    context.signal.addEventListener('abort', dispose, { once: true })
    return { dispose, setInputBlocked() {}, setVisible() {} }
  },
} })
declare global { interface Window { __FE09_TEST__: {
  status: typeof auth.read; counters: typeof counters
  logout: () => Promise<boolean>; openLogin: () => Promise<void>
  dispose: () => void
} } }
window.__FE09_TEST__ = { status: auth.read, counters, logout: auth.logout,
  openLogin: async () => { await router.navigate('/login') }, dispose: () => { router.dispose(); auth.dispose() } }
void router.navigate('/login?returnTo=%2Ffeed', { replace: true })
createRoot(document.getElementById('root')!).render(<StrictMode><RouterProvider router={router} /></StrictMode>)
