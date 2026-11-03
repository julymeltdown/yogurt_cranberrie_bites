import { createRoot } from 'react-dom/client'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { MeHub, ProfileForm } from '../../ui/MePages'
import { SettingsPage } from '../../../settings'
import { participant, setup } from '../fixtures'

const params = new URLSearchParams(location.search)
const route = params.get('route') ?? 'hub', state = params.get('state') ?? 'loaded'
const saved = { ...participant(), profile: { ...participant().profile, bio: state === 'empty' ? null : 'Here for the music.\nSee you in the world.' } }
if (state === 'long') { saved.profile.display_name = 'A'.repeat(40); saved.profile.bio = 'B'.repeat(160) }
const { controller, auth } = setup(async (input, init) => {
  const method = input instanceof Request ? input.method : init?.method
  if (state === 'pending' || (state === 'saving' && method === 'PATCH')) return new Promise<Response>(() => {})
  if (state === 'error') return Response.json({ code: 'UNAVAILABLE', message: 'Fixture unavailable' }, { status: 503 })
  if (state === 'conflict' && method === 'PATCH') return Response.json({ code: 'CONFLICT', message: 'Fixture conflict', details: { reason: 'VERSION_CONFLICT', current_revision: 2 } }, { status: 409 })
  return Response.json(saved)
})
if (state !== 'unavailable') {
  const loading = controller.load()
  if (state !== 'pending') await loading
}
if (['draft', 'conflict', 'saving'].includes(state)) controller.editProfile({ bio: 'Unsaved public bio' })
if (state === 'conflict') await controller.save('profile')
if (state === 'saving') void controller.save('profile')
if (state === 'expired') auth.privacy.changePolicy(auth.privacy.capture(), 'consent-revoked')

createRoot(document.getElementById('root')!).render(<MemoryRouter initialEntries={[route === 'profile' ? '/me/profile' : route === 'settings' ? '/me/settings' : '/me']}><header className="fixture-label">UIV04 · {route} / {state} · deterministic fixtures. No live server or approved avatar source.</header>
  <main><Routes><Route path="/me" element={<MeHub controller={controller} auth={auth} />} /><Route path="/me/profile" element={<ProfileForm controller={controller} />} /><Route path="/me/settings" element={<SettingsPage controller={controller} auth={auth} />} /></Routes></main>
  <style>{`*{box-sizing:border-box}body{margin:0;background:#171616;color:#eee;font:16px/1.5 system-ui}a{color:inherit}.fixture-label{padding:12px;font:12px/1.4 system-ui;color:#d6d0c7}main{padding:12px;max-width:720px;margin:auto}`}</style>
</MemoryRouter>)
window.__UIV04__ = { controller }
declare global { interface Window { __UIV04__: { controller: typeof controller } } }
