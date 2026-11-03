// Test-only actual editor/controller/shared compositor; no product fixture entry or artwork fallback.
import { createRoot } from 'react-dom/client'
import participant from '../../../../../../contracts/fixtures/core/private-participant.json'
import source from '../../../../../../contracts/fixtures/assets/avatar-manifest.json'
import { QueryClient } from '@tanstack/react-query'
import { createMemorySession, createHttpClient } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createAssetResources } from '../../../../world/map/resources'
import { canonicalJson } from '../../../../world/map/json'
import { validateManifest, type AvatarManifest } from '../../../../entities/avatar/runtime/contract'
import { loadAtlas } from '../../../../entities/avatar/runtime/resources'
import { createAvatarCompositor, type AvatarCompositor } from '../../../../entities/avatar/runtime/compositor'
import { createReviewedAvatarPreview, compositorPreview } from '../../preview'
import { AvatarEditor } from '../../AvatarEditor'
import { createAvatarEditorController } from '../../controller'
import { copyConfig, type AvatarConfig } from '../../contract'
import { optionManifest } from './data'

const unavailable = new URLSearchParams(location.search).get('state') === 'unavailable'
const metadata = unavailable ? null : await (await fetch('/__fe13_options/metadata')).json() as { sha256: string }
const document: AvatarManifest = metadata ? await optionManifest(metadata.sha256) : source as AvatarManifest
const resources = createAssetResources({ origin: location.origin, allow: metadata ? [{ path: document.atlas.texture.path!, sha256: metadata.sha256, kind: 'png' }] : [] })
const session = createMemorySession(); session.replace(0, { userId: participant.user_id, eventId: participant.event_id, accessToken: 'option-fixture-token' })
const query = new QueryClient(), privacy = createQueryPrivacy({ client: query, session }), http = createHttpClient({ session })
const compositors: AvatarCompositor[] = [], canvases = new Set<HTMLCanvasElement>(), pending: (() => void)[] = []
let owners = 0, atlasDisposed = 0, hold = false, active = 0, peakActive = 0, reads = 0
const controller = createAvatarEditorController({ auth: { privacy, request: http.request }, async createPreview(_scope, signal) {
  const checkedResources = { async read(...args: Parameters<typeof resources.read>) { reads++; return resources.read(...args) } }
  if (unavailable) return createReviewedAvatarPreview({ document, pin: document, resources: checkedResources, signal })
  const manifest = await validateManifest(canonicalJson(document), document), atlas = await loadAtlas(manifest, checkedResources, signal)
  const compositor = createAvatarCompositor({ manifest, image: atlas.image, bakedSkinToneId: 'tone_fixture', disposeAtlas() { atlasDisposed++; atlas.dispose() } })
  compositors.push(compositor); owners++
  const port = compositorPreview(compositor, 'tone_fixture', signal)
  return { ...port, async render(config, flight) {
    active++; peakActive = Math.max(peakActive, active)
    try {
      const lease = await port.render(config, flight); canvases.add(lease.canvas)
      // Deliberately noncooperative; the controller must keep the serial slot and discard late pixels.
      if (hold) await new Promise<void>(resolve => pending.push(resolve))
      return lease
    } finally { active-- }
  } }
} })
const root = createRoot(window.document.getElementById('root')!); root.render(<AvatarEditor controller={controller} />)
function samePixels(config: AvatarConfig, canvas: HTMLCanvasElement) {
  const reference = compositors.at(-1)!.thumbnail(config, { direction: 'down', animation: 'idle', frame: 0 })
  const a = canvas.getContext('2d')!.getImageData(0, 0, 64, 64).data, b = reference.canvas.getContext('2d')!.getImageData(0, 0, 64, 64).data
  const equal = a.every((value, index) => value === b[index]); reference.dispose(); return equal
}
const api = {
  state: () => ({ editor: controller.read(), owners, atlasDisposed, active, peakActive, reads, pending: pending.length,
    aliveCanvases: [...canvases].filter(c => c.width > 0).length, attachedOptions: window.document.querySelectorAll('.mcr-avatar-option canvas').length,
    caches: compositors.map(c => c.read()) }),
  optionPixels() {
    const draft = controller.read().draft!, category = window.document.querySelector('[role="tab"][aria-selected="true"]')?.textContent?.toLowerCase()
    return [...window.document.querySelectorAll<HTMLButtonElement>('.mcr-avatar-option')].filter(button => button.querySelector('canvas')).map(button => {
      const id = button.querySelector<HTMLElement>('.mcr-avatar-option-name')!.title, canvas = button.querySelector('canvas')!
      const config = category === 'presets' ? document.presets.find(p => p.preset_id === id)!.config : category === 'tones' ? { ...draft, skin_tone_id: id }
        : { ...draft, item_ids: { ...draft.item_ids, [category!]: id === '@none' ? null : id } }
      return { id, equal: samePixels(copyConfig(config), canvas), mainCanvas: canvas === controller.read().preview.canvas,
        marker: [...canvas.getContext('2d')!.getImageData(40, 32, 1, 1).data] }
    })
  },
  mainPixels() { return !!controller.read().preview.canvas && samePixels(controller.read().draft!, controller.read().preview.canvas!) },
  hold(value: boolean) { hold = value }, release() { hold = false; pending.splice(0).forEach(resolve => resolve()) },
  suspend() { privacy.changePolicy(privacy.capture(), 'suspended') },
  async dispose() { root.unmount(); controller.dispose(); privacy.dispose(); query.clear() },
}
Object.assign(window, { __FE13_OPTIONS__: api })
declare global { interface Window { __FE13_OPTIONS__: typeof api } }
