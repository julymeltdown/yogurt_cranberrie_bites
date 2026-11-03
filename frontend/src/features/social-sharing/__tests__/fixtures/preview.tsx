// Isolated qualification fixture, never imported by product composition.
import { StrictMode, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { HandleSettings, SharingOffer, SharingRequest, SharingRequests } from '../../ui/SocialSharing'
import { recipientId, request, requesterId, setup } from './data'
const fixture = setup(globalThis.fetch.bind(globalThis)), { controller, auth } = fixture
declare global { interface Window { __FE31__: { switchActor(): void; expire(): void; policy(): void; logout(): void; counts(): { queries: number; mutations: number }; draft(): string } } }
window.__FE31__ = {
  switchActor() { const userId = auth.session.read().userId === requesterId ? recipientId : requesterId; auth.session.replace(auth.session.read().generation, { userId, eventId: request.event_id, accessToken: userId === requesterId ? 'social-test-only-token' : 'social-recipient-token' }) },
  expire() { fixture.setNow(Date.parse(request.expires_at)) },
  policy() { auth.privacy.changePolicy(auth.privacy.capture(), 'block') },
  logout() { auth.session.replace(auth.session.read().generation, null) },
  counts() { return { queries: auth.queryClient.getQueryCache().getAll().length, mutations: auth.queryClient.getMutationCache().getAll().length } },
  draft() { return controller.read().handleDrafts.INSTAGRAM },
}
function Preview() {
  const [view, setView] = useState<'handles' | 'offer' | 'requests' | 'detail'>('handles')
  const select = (next: typeof view) => {
    controller.hideSecret(); setView(next)
    if (next === 'handles') void controller.loadHandles()
    if (next === 'offer') { void controller.loadHandles(); void controller.loadOffer(request.intro_id) }
    if (next === 'requests') void controller.loadRequests()
  }
  return <><nav aria-label="Fixture navigation" style={{ padding: '1rem', display: 'flex', flexWrap: 'wrap', gap: '.5rem' }}>
    <button onClick={() => select('handles')}>Handle settings</button><button onClick={() => select('offer')}>Introduction offer</button><button onClick={() => select('requests')}>Requests list</button>
    <button onClick={() => { window.__FE31__.switchActor(); select('requests') }}>Switch fixture actor</button>
  </nav><main>{view === 'handles' ? <HandleSettings controller={controller} /> : view === 'offer' ? <><SharingOffer controller={controller} retry={() => { void controller.loadOffer(request.intro_id) }} /><button onClick={() => { const id = controller.read().selectedId; if (id) { setView('detail'); void controller.loadRequest(id) } }}>Open created request</button></> : view === 'requests' ? <SharingRequests controller={controller} onRequest={id => { setView('detail'); void controller.loadRequest(id) }} /> : <SharingRequest controller={controller} retry={() => { void controller.loadRequest() }} />}</main></>
}
void controller.loadHandles()
createRoot(document.getElementById('root')!).render(<StrictMode><Preview /></StrictMode>)
window.addEventListener('pagehide', () => fixture.dispose(), { once: true })
