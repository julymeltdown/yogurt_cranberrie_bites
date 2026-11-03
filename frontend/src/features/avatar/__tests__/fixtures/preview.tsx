// Explicitly synthetic test-only composition. Product adapter is tested separately fail-closed.
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createSessionController } from '../../../auth/model/session'
import { isolatedCookiePolicy } from '../../../auth/__tests__/fixtures/data'
import { createAssetResources } from '../../../../world/map/resources'
import { canonicalJson } from '../../../../world/map/json'
import { validateManifest } from '../../../../entities/avatar/runtime/contract'
import { loadAtlas } from '../../../../entities/avatar/runtime/resources'
import { createAvatarCompositor, type AvatarCompositor } from '../../../../entities/avatar/runtime/compositor'
import { AvatarEditor } from '../../AvatarEditor'
import { createAvatarEditorController } from '../../controller'
import { compositorPreview, createReviewedAvatarPreview } from '../../preview'
import { fixtureManifest } from './data'
import participant from '../../../../../../contracts/fixtures/core/private-participant.json'
const eventId = participant.event_id
import type { AvatarConfig, AvatarState } from '../../contract'

const png = new Uint8Array(await (await fetch('/world/fe13-test/avatar.png', { credentials: 'omit', referrerPolicy: 'no-referrer', cache: 'no-store' })).arrayBuffer()), document = await fixtureManifest(png)
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy() })
auth.start()
await new Promise<void>(resolve => { if (auth.read().capabilities !== 'loading') { resolve(); return }; const stop = auth.subscribe(() => { if (auth.read().capabilities !== 'loading') { stop(); resolve() } }) })
if (!await auth.login('fixture@example.test', 'fixture-password')) throw new Error('Canonical fixture login failed')
const resources = createAssetResources({ origin: location.origin, allow: [{ path: document.atlas.texture.path!, sha256: document.atlas.texture.sha256!, kind: 'png' }] })
let production = false, atlasDisposed = 0, slow = false
const compositors: AvatarCompositor[] = [], releasedCanvases: HTMLCanvasElement[] = [], commits: AvatarState[] = []
const controller = createAvatarEditorController({ auth,
  async createPreview(_scope, signal) {
    if (production) return createReviewedAvatarPreview({ document, pin: document, resources, signal })
    const manifest = await validateManifest(canonicalJson(document), document), atlas = await loadAtlas(manifest, resources, signal)
    const compositor = createAvatarCompositor({ manifest, image: atlas.image, bakedSkinToneId: 'tone_fixture', disposeAtlas() { atlasDisposed++; atlas.dispose() } })
    compositors.push(compositor)
    const preview = compositorPreview(compositor, 'tone_fixture', signal)
    return { ...preview, async render(config, signal) {
      const lease = await preview.render(config, signal); releasedCanvases.push(lease.canvas)
      // Deliberately ignores abort to prove the controller cannot reattach a superseded lease.
      if (slow && config.item_ids.accessory === null) await new Promise(resolve => setTimeout(resolve, 350))
      return lease
    } }
  }, onCommitted: state => { commits.push(state) } })
const root = createRoot(window.document.getElementById('root')!); root.render(<StrictMode><AvatarEditor controller={controller} /></StrictMode>)
const api = {
  state: () => ({ editor: controller.read(), caches: compositors.map(c => c.read()), atlasDisposed,
    canvases: releasedCanvases.filter(c => c.width > 0).length, commits }),
  pixels() { const canvas = controller.read().preview.canvas; return canvas ? [...canvas.getContext('2d')!.getImageData(40, 32, 1, 1).data] : [] },
  sharedPixels() {
    const config = controller.read().draft!, preview = controller.read().preview.canvas!, owner = compositors.at(-1)!
    const shared = owner.thumbnail(config, { direction: 'down', animation: 'idle', frame: 0 }), a = preview.getContext('2d')!.getImageData(0, 0, 64, 64).data,
      b = shared.canvas.getContext('2d')!.getImageData(0, 0, 64, 64).data
    const equal = a.every((v, i) => v === b[i]); shared.dispose(); return equal
  },
  panel(open: boolean) { root.render(open ? <StrictMode><AvatarEditor controller={controller} /></StrictMode> : null) },
  selectionChurn(value: boolean) { slow = value },
  production() { production = true; controller.stop(); controller.start() },
  suspend() { auth.privacy.changePolicy(auth.privacy.capture(), 'suspended') },
  draft(config: AvatarConfig) { return controller.canSelect(config) },
  async dispose() { root.unmount(); controller.dispose(); auth.dispose() },
}
Object.assign(window, { __FE13_TEST_ONLY__: api })
declare global { interface Window { __FE13_TEST_ONLY__: typeof api } }
