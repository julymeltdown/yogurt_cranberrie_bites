import { StrictMode, useEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { createSessionController } from '../../auth/model/session'
import { createMeController, MeHub, ProfileForm } from '..'
import { SettingsPage } from '../../settings'
import { eventId, userId } from './fixtures'
const auth = createSessionController({ eventId })
auth.session.replace(0, { userId, eventId, accessToken: 'SYNTHETIC_ONLY' })
const controller = createMeController(auth)
const runtime = { calls: 0, activeSound: false, signals: [] as AbortSignal[], async apply(preferences: { sound_enabled: boolean }, _scope: unknown, signal: AbortSignal) {
  this.calls++; this.activeSound = preferences.sound_enabled; this.signals.push(signal)
  signal.addEventListener('abort', () => { this.activeSound = false }, { once: true })
} }
function App() {
  useEffect(() => { void controller.load() }, [])
  return <><header><p>SYNTHETIC FE34 · native HTTP fixtures. No production art or real server.</p></header><MemoryRouter initialEntries={['/me']}><Routes>
    <Route path="/me" element={<MeHub controller={controller} auth={auth} />} />
    <Route path="/me/profile" element={<ProfileForm controller={controller} />} />
    <Route path="/me/settings" element={<SettingsPage controller={controller} auth={auth} runtime={runtime} />} />
    <Route path="*" element={<p>Destination link verified. Synthetic route fixture only.</p>} />
  </Routes></MemoryRouter><style>{`*{box-sizing:border-box}body{margin:0;font:16px/1.5 system-ui;color:#eee;background:#1a1a1a}header{padding:12px}a{color:inherit}`}</style></>
}
window.__FE34__ = { state: controller.read, controller, revoke() { auth.privacy.changePolicy(auth.privacy.capture(), 'consent-revoked') },
  switchIdentity() { auth.session.replace(auth.session.read().generation, { userId: eventId, eventId, accessToken:'OTHER_SYNTHETIC_ONLY' }) },
  runtime: () => ({ calls: runtime.calls, activeSound: runtime.activeSound, aborted: runtime.signals.map(s => s.aborted) }) }
declare global { interface Window { __FE34__: { state: typeof controller.read; controller: typeof controller; revoke():void; switchIdentity():void; runtime():unknown } } }
createRoot(document.getElementById('root')!).render(<StrictMode><App /></StrictMode>)
