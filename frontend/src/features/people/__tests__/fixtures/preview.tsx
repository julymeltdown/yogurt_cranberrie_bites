// TEST ONLY: isolated routes, synthetic HTTP and rejected C06 metadata. No product composition.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Link, MemoryRouter, Route, Routes, useParams } from 'react-router-dom'
import { createSessionController } from '../../../auth/model/session'
import { isolatedCookiePolicy } from '../../../auth/__tests__/fixtures/data'
import { createAssetResources } from '../../../../world/map/resources'
import { fixtureManifest } from '../../../avatar/__tests__/fixtures/data'
import { People } from '../../People'
import { createPeopleController } from '../../controller'
import { PublicProfile } from '../../../profile/PublicProfile'
import { IntroEditor } from '../../../intro/IntroEditor'
import { createIntroController } from '../../../intro/controller'
import { AgeStatus } from '../../../age-status/AgeStatus'
import { createAgeStatusController } from '../../../age-status/controller'
import source from '../../../../../../contracts/fixtures/domain/public-intro.json'
const auth = createSessionController({ eventId: source.event_id, cookiePolicy: isolatedCookiePolicy() }); auth.start()
await new Promise<void>(resolve => { const stop = auth.subscribe(() => { if (auth.read().capabilities !== 'loading') { stop(); resolve() } }) })
if (!await auth.login('fixture@example.test', 'fixture-password')) throw new Error('Canonical fixture login failed')
let readOnly = false, resolves = 0
const listeners = new Set<() => void>(), access = { read: () => ({ available: true, readOnly }), subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } } }
const options = { auth, access }, people = createPeopleController(options), intro = createIntroController(options), age = createAgeStatusController(options)
const document = await fixtureManifest(new Uint8Array([0])), resources = createAssetResources({ origin: location.origin, allow: [] })
const avatars = { auth, scope: auth.privacy.capture(), async resolve() { resolves++; return { document, pin: document, resources } } }
function Profile() { const { userId = '' } = useParams(); return <PublicProfile controller={people} userId={userId} avatars={avatars} /> }
const root = createRoot(window.document.getElementById('root')!)
root.render(<StrictMode><MemoryRouter initialEntries={['/meet/people']}><nav className="intro-surface" aria-label="Fixture routes"><Link to="/meet/people">People fixture</Link> · <Link to="/meet/intro/edit">Editor fixture</Link> · <Link to="/me/age">Age fixture</Link></nav><Routes>
  <Route path="/meet/people" element={<People controller={people} avatars={avatars} />} /><Route path="/people/:userId" element={<Profile />} /><Route path="/meet/intro/edit" element={<IntroEditor controller={intro} />} /><Route path="/me/age" element={<AgeStatus controller={age} />} />
</Routes></MemoryRouter></StrictMode>)
const api = { state: () => ({ people: people.read(), intro: intro.read(), age: age.read(), resolves }), readOnly(value: boolean) { readOnly = value; listeners.forEach(fn => fn()) }, suspend() { auth.privacy.changePolicy(auth.privacy.capture(), 'suspended') }, dispose() { root.unmount(); people.dispose(); intro.dispose(); age.dispose(); auth.dispose() } }
Object.assign(window, { __FE2930_TEST_ONLY__: api })
declare global { interface Window { __FE2930_TEST_ONLY__: typeof api } }
