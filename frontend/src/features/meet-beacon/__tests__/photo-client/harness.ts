// Deterministic client qualification only. No real producer/venue/photo rights.
import { harness, beaconId } from '../fixtures/harness'
import type { PrepareImage } from '../../../../shared/media/prepare-image'
import type { PreviewPort } from '../../../../shared/media/preview'
import { inspectImage } from '../../../../shared/media/image-policy'
import { createMeetingBeaconController } from '../../model/controller'
const uid = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
const bytes = () => Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aGr8AAAAASUVORK5CYII='), c => c.charCodeAt(0))
const image = () => ({ blob: new Blob([bytes()], { type: 'image/png' }), width: 1, height: 1, sha256: 'a'.repeat(64) })
export const mediaId = uid(700)
export async function photoHarness(options: { prepare?: PrepareImage; previews?: PreviewPort } = {}) {
  let key = 800, serial = 0
  const created: string[] = [], revoked: string[] = []
  const previews = options.previews ?? { create() { const url = `blob:synthetic-meeting-${++serial}`; created.push(url); return url }, revoke(url: string) { revoked.push(url) } }
  const h = await harness({ contract: 'meeting-photo/v1', prepare: options.prepare ?? (async () => image()), previews, newKey: () => uid(key++) })
  // Synthetic permission fixture only; actual FE19 place permission still gates use.
  h.items[1]!.provenance.photo_permission = 'GRANTED'; await h.placesController.refresh()
  let uploaded = bytes(), info = { width: 1, height: 1, type: 'image/png' as 'image/png' | 'image/webp' | 'image/jpeg' }
  let uploadCount = 0, activeMediaId = mediaId
  let readDenied = false, finalize: 'ok' | 'lost' | 'failed' = 'ok', contentGate: Promise<Response> | null = null
  const metadata = () => {
    return { media_id: activeMediaId, state: 'READY', revision: 3, width: info.width, height: info.height, content_type: info.type, purpose: 'BEACON',
      read_path: `/api/concert/events/${h.auth.session.read().eventId}/media/${activeMediaId}/content` }
  }
  h.setOverride(request => {
    const path = new URL(request.url).pathname, base = `/api/concert/events/${h.auth.session.read().eventId}/media`
    if (!path.startsWith(base)) return null
    return (async () => {
    if (path.endsWith('/uploads')) {
      const body = await request.clone().json()
      if (body.purpose !== 'BEACON') throw Error('Fixture rejects substituted photo purpose')
      activeMediaId = uid(700 + uploadCount++)
      return Response.json({ media_id: activeMediaId, state: 'PENDING', upload_path: `${base}/${activeMediaId}/bytes`, upload_method: 'PUT', max_bytes: 10485760, expires_at: '2026-11-08T00:00:00Z', revision: 1 })
    }
    if (path.endsWith('/bytes')) {
      uploaded = new Uint8Array(await request.arrayBuffer())
      info = await inspectImage(new Blob([uploaded], { type: request.headers.get('content-type') ?? 'image/png' }), new AbortController().signal)
      return Response.json({ media_id: activeMediaId, state: 'UPLOADED', revision: 2 })
    }
    if (path.endsWith('/finalize')) {
      if (finalize === 'lost') throw Error('Synthetic unknown finalize outcome')
      if (finalize === 'failed') return Response.json({ code: 'DENIED', message: 'Synthetic unavailable image', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
      return Response.json(metadata())
    }
    if (readDenied) return Response.json({ code: 'DENIED', message: 'Synthetic denied image', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 })
    if (path.endsWith('/content')) return contentGate ?? new Response(uploaded.slice().buffer, { headers: { 'Content-Type': info.type, 'Cache-Control': 'no-store' } })
    return Response.json(metadata())
    })()
  })
  return { ...h, created, revoked, mediaId, metadata,
    denyReads(value = true) { readDenied = value }, finalizeMode(value: typeof finalize) { finalize = value },
    gateContent(value: Promise<Response> | null) { contentGate = value },
    persisted() { return h.beacon },
    reread: () => h.controller.select(beaconId),
    newReader() {
      return createMeetingBeaconController({ auth: h.auth, places: h.placesController, now: h.now,
        authority: { read: () => h.authority, subscribe: () => () => {} },
        photoClient: { contract: 'meeting-photo/v1', previews },
      })
    },
  }
}
