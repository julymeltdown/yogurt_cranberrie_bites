// TEST_ONLY component qualification; this is explicitly not a default-App authority producer.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { MeetingBeaconDetail } from '../../ui/MeetingBeacon'
import { beaconId, harness } from './harness'
import '../../../../shared/styles/base.css'

if (!import.meta.env.DEV) throw Error('PI05 component fixture is development-only')
const h = await harness(), scenario = new URLSearchParams(location.search).get('case') ?? 'current'
await h.auth.checkAdmission()
if (scenario === 'long') h.setBeacon({ ...h.beacon, meeting: { ...h.beacon.meeting,
  instructions: 'SYNTHETIC private guidance with a long identifying description. '.repeat(10) } })
await h.controller.select(beaconId)
if (scenario === 'expired') h.setTime(Date.parse(h.beacon.meeting.expires_at) + 1)
if (scenario === 'private') h.setAuthority(null)
const root = createRoot(document.getElementById('root')!)
root.render(<StrictMode><MeetingBeaconDetail controller={h.controller} ownerActions /></StrictMode>)
const api = { classification: 'TEST_ONLY_COMPONENT_NOT_DEFAULT_APP',
  state: () => ({ phase: h.controller.read().phase, canRead: h.controller.read().canRead, requests: h.requests.map(r => ({ method: r.method, path: new URL(r.url).pathname })) }),
  dispose() { root.unmount(); h.dispose() } }
Object.assign(window, { __PI05_COMPONENT__: api })
