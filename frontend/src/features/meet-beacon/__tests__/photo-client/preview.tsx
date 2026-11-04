import { createRoot } from 'react-dom/client'
import { useSyncExternalStore } from 'react'
import { MeetingBeaconForm, MeetingBeaconDetail } from '../../index'
import type { MeetingBeaconController } from '../../model/controller'
import { prepareImage } from '../../../../shared/media/prepare-image'
import { photoHarness } from './harness'
import { beaconId } from '../fixtures/harness'
const created: string[] = [], revoked: string[] = []
const previews = { create(blob: Blob) { const url = URL.createObjectURL(blob); created.push(url); return url }, revoke(url: string) { revoked.push(url); URL.revokeObjectURL(url) } }
const h = await photoHarness({ prepare: prepareImage, previews })
let controller = h.controller
const root = createRoot(document.getElementById('root')!)
function Preview({ owner }: { owner: MeetingBeaconController }) {
  const state = useSyncExternalStore(owner.subscribe, owner.read)
  return <main style={{ maxWidth: 680, margin: 'auto', background: '#201e24' }}>
    <p style={{ color: '#eee8df', padding: 16, margin: 0 }}>SYNTHETIC CLIENT FIXTURE. Fictional place, audience, identity and server storage. No venue/photo-rights or live producer approval.</p>
    <MeetingBeaconDetail controller={owner} ownerActions />
    <MeetingBeaconForm controller={owner} mode={state.beacon ? 'update' : 'create'} />
  </main>
}
const render = () => root.render(<Preview owner={controller} />); render()
const api = {
  state: () => ({ phase: controller.read().phase, command: controller.read().command.phase, upload: controller.read().photoDraft?.phase,
    photo: controller.read().photo ? { mediaId: controller.read().photo!.mediaId, src: controller.read().photo!.src } : null,
    persisted: h.persisted(), created: [...created], revoked: [...revoked],
    requests: h.requests.map(r => ({ path: new URL(r.url).pathname, method: r.method, key: r.headers.get('idempotency-key') })),
  }),
  async reread() { controller.dispose(); controller = h.newReader(); controller.start(); await controller.select(beaconId); render() },
  revoke() { h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'suspended') },
  expire() { h.setTime(Date.parse(h.persisted().meeting.expires_at)); controller.refreshTime() },
  unknownUpload() { h.finalizeMode('lost') },
  dispose() { root.unmount(); controller.dispose(); h.dispose() },
}
Object.assign(window, { __MEETING_PHOTO__: api })
declare global { interface Window { __MEETING_PHOTO__: typeof api } }
