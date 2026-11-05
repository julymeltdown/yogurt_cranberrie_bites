// DEVELOPMENT ONLY. Actual DefaultAdmin owner/panel; controlled HTTP and synthetic clock.
// Does not mount root main.tsx, a live backend or measured production clock input.
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import { PortsProvider } from '../../../access'
import { DefaultAdminPanel } from '../../panel'
import { compositionFixture } from './fixture'
import '../../../../shared/styles/base.css'
import '../../../shell.css'
if (!import.meta.env.DEV) throw Error('Development qualification only')
const owner = compositionFixture(); owner.auth.start()
let router: ReturnType<typeof createBrowserRouter> | null = null
function Preview() {
  const [ready, setReady] = useState(false), [message, setMessage] = useState('')
  return <div><p>TEST ONLY · actual Default Admin owner/panel · controlled HTTP and clock · not root main or a live service.</p>
    {!ready ? <form className="event-ops" onSubmit={event => {
      event.preventDefault(); const form = new FormData(event.currentTarget)
      void owner.auth.login(String(form.get('email')), String(form.get('password'))).then(async signedIn => {
        if (!signedIn) { setMessage('Synthetic login unavailable'); return }
        await owner.auth.checkAdmission()
        router = createBrowserRouter(['/admin', '/admin/event', '/admin/reports', '/admin/places', '/admin/audit'].map(path => ({ path,
          element: <PortsProvider ports={owner.ports}><DefaultAdminPanel key={path} scope={owner.scope()} /></PortsProvider> })))
        setReady(true)
      })
    }}><label htmlFor="email">Email</label><input id="email" name="email" type="email" required />
      <label htmlFor="password">Password</label><input id="password" name="password" type="password" required />
      <button type="submit">Sign in to controlled fixture</button><p role="status">{message}</p></form>
      : router && <RouterProvider router={router} />}
  </div>
}
const root = createRoot(document.getElementById('root')!); root.render(<Preview />)
const control = { mode: owner.policy.mode, settle: owner.policy.settle,
  role: owner.role, async withdraw() { owner.role(null); await owner.entry()?.refresh() },
  state: () => ({ controller: owner.state()?.servicePolicy?.read() ?? null, requests: owner.policy.requests,
    reportsCurrent: owner.state()?.reports?.isCurrent() ?? false }),
  dispose() { root.unmount(); router?.dispose(); owner.dispose() },
}
Object.assign(window, { __FE38_COMPOSITION__: control })
declare global { interface Window { __FE38_COMPOSITION__: typeof control } }
