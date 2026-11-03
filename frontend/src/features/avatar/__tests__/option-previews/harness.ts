// TEST ONLY: no product artwork. Instrumented leases deliberately simulate noncooperative rendering.
import { QueryClient } from '@tanstack/react-query'
import participant from '../../../../../../contracts/fixtures/core/private-participant.json'
import conflict from '../../../../../../contracts/fixtures/core/error-version-conflict.json'
import { createMemorySession, createHttpClient } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createAvatarEditorController } from '../../controller'
import { copyConfig, layers, type AvatarConfig, type AvatarPreviewPort, type PreviewLease } from '../../contract'
import { original } from '../fixtures/harness'

export const configs: readonly AvatarConfig[] = [original, ...['second', 'third', 'fourth', 'fifth', 'sixth', 'seventh'].map(suffix => copyConfig({ ...original, item_ids: { ...original.item_ids, hair: `hair_${suffix}` } }))]
export const unsupported = copyConfig({ ...original, skin_tone_id: 'tone_unsupported' })
export function instrumentedPort() {
  let disposed = false, held = false, ignoredAbort = false, fail = false, active = 0, peakActive = 0, peakLeases = 0, renders = 0
  const leases = new Set<PreviewLease>(), canvases: HTMLCanvasElement[] = [], pending: (() => void)[] = [], signals: AbortSignal[] = []
  const port: AvatarPreviewPort = {
    catalog: { presets: [...configs.map((config, index) => ({ id: `option_${index + 1}`, config })), { id: 'unsupported', config: unsupported }],
      items: [...layers.map(layer => ({ id: original.item_ids[layer]!, layer })), ...configs.slice(1).map(config => ({ id: config.item_ids.hair, layer: 'hair' as const }))], tones: [original.skin_tone_id, 'tone_unsupported'] },
    supports(config) { return !disposed && config.manifest_id === original.manifest_id && config.manifest_version === original.manifest_version
      && config.skin_tone_id === original.skin_tone_id && layers.every(layer => layer === 'hair' ? configs.some(c => c.item_ids.hair === config.item_ids.hair)
        : config.item_ids[layer] === original.item_ids[layer] || layer === 'accessory' && config.item_ids.accessory === null) },
    render(config, signal) {
      renders++; active++; peakActive = Math.max(peakActive, active); signals.push(signal)
      if (fail) { active--; return Promise.reject(new Error('Unavailable')) }
      const canvas = document.createElement('canvas'); canvas.width = canvas.height = 64; canvas.dataset.testHair = config.item_ids.hair; canvases.push(canvas)
      const lease: PreviewLease = { canvas, dispose() { canvas.width = canvas.height = 0; canvas.remove(); leases.delete(lease) } }
      leases.add(lease); peakLeases = Math.max(peakLeases, leases.size)
      return new Promise((resolve, reject) => {
        let settled = false
        const finish = () => { if (settled) return; settled = true; active--; signal.removeEventListener('abort', abort); resolve(lease) }
        const abort = () => { if (ignoredAbort || settled) return; settled = true; active--; lease.dispose(); reject(new Error('Aborted')) }
        signal.addEventListener('abort', abort, { once: true })
        if (held) pending.push(finish); else finish()
      })
    },
    dispose() { disposed = true; for (const lease of leases) lease.dispose() },
  }
  return { port, leases, canvases, pending, signals, get disposed() { return disposed }, get renders() { return renders },
    get active() { return active }, get peakActive() { return peakActive }, get peakLeases() { return peakLeases },
    set held(value: boolean) { held = value }, set ignoredAbort(value: boolean) { ignoredAbort = value }, set fail(value: boolean) { fail = value } }
}
export function optionHarness() {
  const session = createMemorySession(); session.replace(0, { userId: participant.user_id, eventId: participant.event_id, accessToken: 'test-token' })
  const query = new QueryClient(), privacy = createQueryPrivacy({ client: query, session }), ports: ReturnType<typeof instrumentedPort>[] = []
  const requests: { method: string; body: unknown }[] = []
  let mode: 'ok' | 'conflict' | 'uncertain' = 'ok', server = { revision: 1, config: original }
  const http = createHttpClient({ session, fetch: async input => {
    const request = input as Request, body = request.method === 'GET' ? null : await request.json(); requests.push({ method: request.method, body })
    if (request.method === 'GET') return Response.json({ ...participant, profile: { ...participant.profile, avatar: server } })
    if (mode === 'conflict') return Response.json(conflict, { status: 409 })
    const command = body as { expected_revision: number; config: AvatarConfig }; server = { revision: command.expected_revision + 1, config: command.config }
    if (mode === 'uncertain') throw new TypeError('Uncertain fixture response')
    return Response.json(server)
  } })
  const controller = createAvatarEditorController({ auth: { privacy, request: http.request }, createPreview: async () => { const owner = instrumentedPort(); ports.push(owner); return owner.port } })
  return { controller, session, privacy, ports, requests, setMode(value: typeof mode) { mode = value },
    setServer(value: typeof server) { server = value }, dispose() { controller.dispose(); privacy.dispose(); query.clear() } }
}
