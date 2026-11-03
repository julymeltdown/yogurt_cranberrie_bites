// Deterministic HTTP/identity only. Actual published runtime, selected-source artwork and editor.
import { createRoot } from 'react-dom/client'
import { QueryClient } from '@tanstack/react-query'
import participant from '../../../../../../contracts/fixtures/core/private-participant.json'
import manifest from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { createMemorySession, createHttpClient } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createWholeCharacterRuntime } from '../../../../entities/avatar/whole-character'
import { createWholeCharacterEditorController } from '../controller'
import { WholeCharacterEditor } from '../WholeCharacterEditor'
import type { WholeCharacterConfig } from '../model'

const session = createMemorySession()
session.replace(0, { userId: participant.user_id, eventId: participant.event_id, accessToken: 'whole-fixture-token' })
const query = new QueryClient(), privacy = createQueryPrivacy({ client: query, session }), http = createHttpClient({ session })
const runtime = createWholeCharacterRuntime({ privacy, manifest, pin: manifest, origin: location.origin })
const committed: unknown[] = [], controller = createWholeCharacterEditorController({ auth: { privacy, request: http.request }, scope: privacy.capture(), runtime, onCommitted: value => committed.push(value) })
const root = createRoot(document.getElementById('root')!)
root.render(<WholeCharacterEditor controller={controller} />)
const api = {
  state: () => ({ editor: controller.read(), runtime: runtime.diagnostics(), committed }),
  async mode(value: string) { await fetch('/__whole/control?mode=' + value, { method: 'POST' }) },
  async pixels() {
    const checks: { id: string; equal: boolean }[] = []
    for (const canvas of document.querySelectorAll<HTMLCanvasElement>('.mcr-whole-editor canvas')) {
      const button = canvas.closest('button'), label = button?.getAttribute('aria-label')?.replace('Choose ', '')
      const choice = label ? controller.read().choices.find(c => c.label === label) : undefined
      const config = (choice?.config ?? controller.read().draft) as WholeCharacterConfig
      const abort = new AbortController(), lease = await runtime.acquire(config, { signal: abort.signal }), render = lease.read()
      if (!render) throw new Error('Missing genuine render')
      const reference = document.createElement('canvas'); reference.width = canvas.width; reference.height = canvas.height
      const context = reference.getContext('2d')!; context.imageSmoothingEnabled = false
      context.drawImage(render.image, render.source.x, render.source.y, render.source.width, render.source.height, 0, 0, reference.width, reference.height)
      const actual = canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height).data
      const expected = context.getImageData(0, 0, reference.width, reference.height).data
      checks.push({ id: config.preset_id, equal: actual.every((v, i) => v === expected[i]) }); abort.abort(); lease.dispose(); reference.width = reference.height = 0
    }
    return checks
  },
  stop() { controller.stop() },
  start() { controller.start() },
  suspend() { privacy.changePolicy(privacy.capture(), 'suspended') },
  logout() { session.replace(session.read().generation, null) },
  dispose() { root.unmount(); controller.dispose(); runtime.dispose(); privacy.dispose(); query.clear() },
}
Object.assign(window, { __MCR_WHOLE_EDITOR__: api })
declare global { interface Window { __MCR_WHOLE_EDITOR__: typeof api } }
