// DEVELOPMENT ONLY: real Auth/login, actual OperationsEntry and leaf; controlled HTTP/server clock.
// This fixture is not the default Admin composition or a live production backend cohort.
import { createRoot } from 'react-dom/client'
import { useState } from 'react'
import { ServicePolicyOperations } from '../ServicePolicyOperations'
import { fixture } from './fixture'
import '../../../../shared/styles/base.css'
import '../../event/ui/operations.css'
if (!import.meta.env.DEV) throw Error('Development qualification only')
const owner = fixture()
owner.auth.start()
function Preview() {
  const [ready, setReady] = useState(false), [message, setMessage] = useState('')
  return <main className="event-ops"><h1>Controlled service policy qualification</h1>
    <p>TEST ONLY · controlled HTTP and server clock · not the default Admin composition or a live service.</p>
    {!ready ? <form onSubmit={event => {
      event.preventDefault(); const form = new FormData(event.currentTarget)
      void owner.auth.login(String(form.get('email')), String(form.get('password'))).then(async signedIn => {
        if (!signedIn) { setMessage('Synthetic login unavailable'); return }
        await owner.initialize(); setReady(true)
      })
    }}><label htmlFor="email">Email</label><input id="email" name="email" type="email" required />
      <label htmlFor="password">Password</label><input id="password" name="password" type="password" required />
      <button type="submit">Sign in to controlled fixture</button><p role="status">{message}</p></form>
      : <ServicePolicyOperations controller={owner.controller} />}
  </main>
}
const root = createRoot(document.getElementById('root')!); root.render(<Preview />)
const control = { mode: owner.mode, settle: owner.settle, withdraw: owner.withdraw, state: () => ({ controller: owner.controller.read(), requests: owner.requests }),
  dispose() { root.unmount(); owner.dispose() } }
Object.assign(window, { __FE38_POLICY__: control })
declare global { interface Window { __FE38_POLICY__: typeof control } }
