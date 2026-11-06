import { vi } from 'vitest'
import { createMemorySession } from '../../../../shared/api/http'
import { createImageUploadController } from '../upload-controller'
import type { MediaUploadTransport } from '../../../../shared/media/transport'
import type { CommandBinding, MediaReceipt } from '../../../../shared/media/contract'
import type { PrepareImage } from '../../../../shared/media/prepare-image'
export const uid = (n: number) => `00000000-0000-4000-8000-${n.toString().padStart(12, '0')}`
export const user = uid(1), event = uid(2), mediaId = uid(3), now = Date.parse('2026-11-04T00:00:00Z')
export const bytes = () => Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aGr8AAAAASUVORK5CYII='), c => c.charCodeAt(0))
export const file = () => new File([bytes()], 'private-local-name.png', { type: 'image/png' })
export const image = () => ({ blob: new Blob([bytes()], { type: 'image/png' }), width: 1, height: 1, sha256: 'a'.repeat(64) })
export const intent = () => ({ media_id: mediaId, state: 'PENDING' as const, upload_path: `/api/concert/events/${event}/media/${mediaId}/bytes`, max_bytes: 10485760 as const,
  expires_at: '2099-01-01T00:00:00Z', upload_method: 'PUT' as const, revision: 1 })
export const ready = () => ({ media_id: mediaId, state: 'READY' as const, revision: 3, width: 1, height: 1, content_type: 'image/png' as const,
  purpose: 'POST' as const, read_path: `/api/concert/events/${event}/media/${mediaId}/content` })
export const receipt = (b: CommandBinding, patch: Partial<MediaReceipt> = {}): MediaReceipt => ({ receipt_id: uid(4), event_id: event, actor_id: user,
  operation_id: b.operation, idempotency_key: b.key, state: 'PENDING', resource_id: null, resource_revision: null, failure_reason: null,
  created_at: '2026-11-04T00:00:00Z', expires_at: '2026-11-05T00:00:00Z', result_status: null, ...patch })
export function deferred<T>() { let resolve!: (v: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
export function setup(patch: Partial<MediaUploadTransport> = {}, prepare: PrepareImage = async () => image()) {
  const session = createMemorySession(); session.replace(0, { userId: user, eventId: event, accessToken: 'memory' })
  const transport: MediaUploadTransport = {
    initialize: vi.fn(async () => ({ kind: 'result' as const, value: intent() })),
    upload: vi.fn(async () => ({ kind: 'result' as const, value: { media_id: mediaId, state: 'UPLOADED' as const, revision: 2 } })),
    finalize: vi.fn(async () => ({ kind: 'result' as const, value: ready() })),
    status: vi.fn(async b => receipt(b)), read: vi.fn(async () => ready()), ...patch,
  }
  let k = 10
  const previews = { create: vi.fn(() => `blob:fixture-${k}`), revoke: vi.fn() }
  const controller = createImageUploadController({ session, transport, prepare, previews, purpose: 'POST', now: () => now, key: () => uid(k++) })
  return { session, transport, controller, previews }
}
