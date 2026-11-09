import { QueryClient } from '@tanstack/react-query'
import participant from '../../../../../../contracts/fixtures/core/private-participant.json'
import conflict from '../../../../../../contracts/fixtures/core/error-version-conflict.json'
import { createMemorySession, createHttpClient } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createAvatarEditorController } from '../../controller'
import { copyConfig, decodeAvatar, layers, type AvatarConfig, type AvatarPreviewPort, type AvatarState, type PreviewLease } from '../../contract'
import { deferred } from '../../../../test-support/fixtures'
export { deferred }

export const userId = participant.user_id, eventId = participant.event_id
export const original: AvatarConfig = decodeAvatar(participant.profile.avatar).config
export const alternate: AvatarConfig = copyConfig({ ...original, item_ids: { ...original.item_ids, hair: 'hair_second' } })
export const unsupported: AvatarConfig = copyConfig({ ...original, skin_tone_id: 'tone_unsupported' })
export type Mode = 'ok' | 'conflict' | 'denied' | 'uncertain' | 'invalid' | 'slow' | 'stale-read' | 'read-failed' | 'other-user'
/** Explicit test-only port. It draws no character art and has no production import. */
export function testPreview() {
  const leases = new Set<HTMLCanvasElement>(), pending: ReturnType<typeof deferred<PreviewLease>>[] = []
  let disposed = false, slow = false, renderCount = 0
  const port: AvatarPreviewPort = {
    catalog: { presets: [{ id: 'fixture_original', config: original }, { id: 'fixture_alternate', config: alternate }, { id: 'fixture_unsupported', config: unsupported }],
      items: [...layers.map(layer => ({ id: original.item_ids[layer]!, layer })), { id: 'hair_second', layer: 'hair' }], tones: ['tone_fixture', 'tone_unsupported'] },
    supports: config => !disposed && config.manifest_id === original.manifest_id && config.manifest_version === original.manifest_version
      && config.skin_tone_id === original.skin_tone_id && layers.every(layer => config.item_ids[layer] === original.item_ids[layer]
        || (layer === 'hair' && config.item_ids[layer] === 'hair_second') || (layer === 'accessory' && config.item_ids[layer] === null)),
    async render(_config, _signal) {
      renderCount++
      const canvas = document.createElement('canvas'); canvas.width = canvas.height = 64
      const lease = { canvas, dispose() { canvas.width = canvas.height = 0; canvas.remove(); leases.delete(canvas) } }; leases.add(canvas)
      if (slow) { const flight = deferred<PreviewLease>(); pending.push(flight); return flight.promise }
      return lease
    },
    dispose() { disposed = true; for (const canvas of leases) { canvas.width = canvas.height = 0; canvas.remove() }; leases.clear() },
  }
  return { port, leases, pending, get disposed() { return disposed }, get renders() { return renderCount }, set slow(value: boolean) { slow = value } }
}
export function harness() {
  const session = createMemorySession(); session.replace(0, { userId, eventId, accessToken: 'fixture-token' })
  const query = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } }), privacy = createQueryPrivacy({ client: query, session })
  let mode: Mode = 'ok', server: AvatarState = { revision: 1, config: original }
  const requests: { method: string; path: string; body: unknown; signal: AbortSignal }[] = [], previews: ReturnType<typeof testPreview>[] = [], commits: AvatarState[] = []
  const pendingSave = deferred<Response>()
  const http = createHttpClient({ session, fetch: async input => {
    const request = input as Request, path = new URL(request.url).pathname, body = request.method === 'GET' ? null : await request.json()
    requests.push({ method: request.method, path, body, signal: request.signal })
    if (request.method === 'GET') {
      if (mode === 'read-failed') return Response.json({}, { status: 503 })
      return Response.json({ ...participant, user_id: mode === 'other-user' ? '00000000-0000-4000-8000-000000000102' : userId,
        profile: { ...participant.profile, avatar: mode === 'stale-read' ? { ...server, revision: 1 } : server } })
    }
    if (mode === 'slow') return pendingSave.promise
    if (mode === 'conflict') return Response.json(conflict, { status: 409 })
    if (mode === 'denied') return Response.json({ code: 'GATEWAY_403', message: 'private server message', details: { reason: 'POLICY_DENIED' } }, { status: 403 })
    const command = body as { config: AvatarConfig; expected_revision: number }
    server = { revision: command.expected_revision + 1, config: command.config }
    if (mode === 'uncertain') throw new TypeError('private network error')
    if (mode === 'invalid') return Response.json({ ...server, revision: command.expected_revision })
    return Response.json(server)
  } })
  const controller = createAvatarEditorController({ auth: { privacy, request: http.request }, createPreview: async () => { const preview = testPreview(); previews.push(preview); return preview.port }, onCommitted: avatar => commits.push(avatar) })
  return { controller, session, privacy, query, requests, previews, commits, pendingSave,
    setMode(value: Mode) { mode = value }, setServer(value: AvatarState) { server = value }, get server() { return server },
    dispose() { controller.dispose(); privacy.dispose(); query.clear() } }
}

export { participant }
