import type { HttpRequest, HttpResult } from '../api/http'
import { decodeIntent, decodeMedia, decodeReceipt, decodeUploaded, mediaBase,
  type CommandBinding, type MediaCommandResult, type MediaReceipt, type UploadedBytes, type UploadIntent } from './contract'
import type { MediaPurpose, PreparedImage, ReadyMedia } from './types'
/** Pass canonical SessionController.request. No alternate token/cookie plumbing. */
export type MediaRequest = <T>(input: Omit<HttpRequest<T>, 'refresh'>) => Promise<HttpResult<T>>
export interface MediaUploadTransport {
  initialize(binding: CommandBinding, purpose: MediaPurpose, image: PreparedImage, signal: AbortSignal): Promise<MediaCommandResult<UploadIntent>>
  upload(binding: CommandBinding, intent: UploadIntent, image: PreparedImage, signal: AbortSignal, onProgress: (percent: number | null) => void): Promise<MediaCommandResult<UploadedBytes>>
  finalize(binding: CommandBinding, purpose: MediaPurpose, revision: number, signal: AbortSignal): Promise<MediaCommandResult<ReadyMedia>>
  status(binding: CommandBinding, signal: AbortSignal): Promise<MediaReceipt>
  read(binding: CommandBinding, purpose: MediaPurpose, signal: AbortSignal): Promise<ReadyMedia>
}
export function createMediaUploadTransport(request: MediaRequest): MediaUploadTransport {
  async function command<T>(b: CommandBinding, path: string, body: HttpRequest<T>['body'], signal: AbortSignal,
    decode: (v: unknown) => T): Promise<MediaCommandResult<T>> {
    return (await request({ path, method: b.operation === 'putConcertUploadBytes' ? 'PUT' : 'POST', access: 'protected', errorFamily: 'domain',
      domainOperation: b.operation, idempotencyKey: b.key, ...(body ? { body } : {}), signal,
      decode: (v, c): MediaCommandResult<T> => {
        if (c.path !== path || c.scope.generation !== b.scope.generation || c.scope.userId !== b.scope.userId || c.scope.eventId !== b.scope.eventId) throw new Error('Invalid media context')
        if (c.status === 202) {
          const r = decodeReceipt(v, b); if (r.state !== 'PENDING') throw new Error('Invalid pending receipt')
          return Object.freeze({ kind: 'pending', receipt: r })
        }
        if (c.status !== 200) throw new Error('Invalid media status')
        return Object.freeze({ kind: 'result', value: decode(v) })
      },
    })).data
  }
  return {
    initialize: (b, purpose, image, signal) => command(b, `${mediaBase(b.scope)}/uploads`, {
      json: { purpose, content_type: image.blob.type, byte_length: image.blob.size, sha256: image.sha256 },
    }, signal, v => decodeIntent(v, b.scope)),
    upload(b, intent, image, signal, progress) {
      const i = decodeIntent(intent, b.scope)
      if (i.media_id !== b.mediaId || image.blob.size > i.max_bytes) return Promise.reject(new Error('Invalid upload intent'))
      progress(null) // Fetch cannot report actual upload transfer progress.
      return command(b, i.upload_path, { binary: image.blob }, signal, v => decodeUploaded(v, i.media_id))
    },
    finalize: (b, purpose, revision, signal) => command(b, `${mediaBase(b.scope)}/${b.mediaId}/finalize`, {
      json: { expected_revision: revision },
    }, signal, v => decodeMedia(v, b.scope, b.mediaId!, purpose)),
    async status(b, signal) {
      const path = `/api/concert/events/${b.scope.eventId}/commands`
      return (await request({ path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertCommandStatus',
        query: { operation_id: b.operation, idempotency_key: b.key }, signal,
        decode: (v, c) => {
          if (c.path !== path || c.status !== 200 || c.scope.generation !== b.scope.generation || c.scope.userId !== b.scope.userId || c.scope.eventId !== b.scope.eventId) throw new Error('Invalid media context')
          return decodeReceipt(v, b)
        },
      })).data
    },
    async read(b, purpose, signal) {
      const path = `${mediaBase(b.scope)}/${b.mediaId}`
      return (await request({ path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertMedia', signal,
        decode: (v, c) => {
          if (c.path !== path || c.status !== 200 || c.scope.generation !== b.scope.generation || c.scope.userId !== b.scope.userId || c.scope.eventId !== b.scope.eventId) throw new Error('Invalid media context')
          return decodeMedia(v, b.scope, b.mediaId!, purpose)
        },
      })).data
    },
  }
}
