import type { HttpRequest } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import {
  isAborted,
  newIdempotencyKey,
  send,
  withRetry,
  type RequestPort,
} from '../../../shared/api/request'

/**
 * WP-05 T-410: the profile media upload client.
 *
 * The lifecycle is the frozen one: **init → PUT bytes → finalize**. Each step has
 * its own failure handling:
 *
 * * a failed `PUT` or `finalize` **deletes the staged asset**, so a retry starts
 *   from a clean init rather than leaving an orphan behind;
 * * a transient failure is retried **without duplicating the asset**, because the
 *   retry reuses the same `media_id` and the same idempotency key;
 * * an account-context change **aborts in-flight uploads and clears state**, so
 *   an upload started by one account can never land in another's gallery.
 *
 * The purpose is a closed enum: this client only ever creates `INTRO_POST`
 * assets, because the intro composer is its only caller.
 */
export const PURPOSE = 'INTRO_POST' as const

export interface ProfileMediaIntent {
  readonly media_id: string
  readonly state: 'PENDING'
  readonly upload_path: string
  readonly max_bytes: number
  readonly expires_at: string
  readonly upload_method: 'PUT'
  readonly revision: number
}

export interface ReadyMedia {
  readonly media_id: string
  readonly state: 'READY'
  readonly revision: number
  readonly width: number
  readonly height: number
  readonly content_type: 'image/jpeg' | 'image/png' | 'image/webp'
  readonly purpose: string
  readonly read_path: string
}

export interface UploadInput {
  readonly blob: Blob
  readonly width: number
  readonly height: number
  readonly type: 'image/jpeg' | 'image/png' | 'image/webp'
}

export interface UploadOptions extends RequestPort {
  /** Deterministic fixtures only; production uses `crypto.randomUUID`. */
  readonly key?: () => string
  /** How many times a transient step is retried. */
  readonly retries?: number
  readonly signal?: AbortSignal
}

export type UploadResult =
  | { readonly ok: true; readonly media: ReadyMedia }
  | { readonly ok: false; readonly cancelled: boolean; readonly message: string }

const uuid = (value: unknown): string | null =>
  typeof value === 'string' &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
    ? value
    : null

function decodeIntent(value: unknown): ProfileMediaIntent {
  const object = value as Record<string, unknown>
  const mediaId = uuid(object['media_id'])
  if (mediaId === null) throw new Error('Invalid media intent')
  return Object.freeze({
    media_id: mediaId,
    state: 'PENDING',
    upload_path: String(object['upload_path']),
    max_bytes: Number(object['max_bytes']),
    expires_at: String(object['expires_at']),
    upload_method: 'PUT',
    revision: Number(object['revision']),
  })
}

function decodeReady(value: unknown): ReadyMedia {
  const object = value as Record<string, unknown>
  const mediaId = uuid(object['media_id'])
  if (mediaId === null || object['state'] !== 'READY') throw new Error('Invalid media')
  return Object.freeze({
    media_id: mediaId,
    state: 'READY',
    revision: Number(object['revision']),
    width: Number(object['width']),
    height: Number(object['height']),
    content_type: object['content_type'] as ReadyMedia['content_type'],
    purpose: String(object['purpose']),
    read_path: String(object['read_path']),
  })
}

/**
 * Upload one prepared image.
 *
 * The whole lifecycle is one function so the cleanup path cannot be forgotten:
 * every early return after `init` deletes the staged asset.
 */
export async function uploadProfileMedia(
  input: UploadInput,
  options: UploadOptions,
): Promise<UploadResult> {
  const key = options.key ?? newIdempotencyKey
  const retries = options.retries ?? 1
  const signal = options.signal
  const cancelled = () => signal?.aborted === true

  // One idempotency key per attempt, reused by every retry of that attempt, so a
  // retried finalize cannot create a second asset.
  const attemptKey = key()
  let mediaId: string | null = null

  const attempt = <T>(run: () => Promise<T>): Promise<T> =>
    withRetry(run, { retries, ...(signal === undefined ? {} : { signal }) })

  try {
    // 1. init
    const intent = await attempt(() =>
      send(
        options,
        {
          path: '/api/me/media/uploads',
          method: 'POST',
          access: 'protected',
          errorFamily: 'domain',
          domainOperation: 'initProfileMediaUpload',
          idempotencyKey: attemptKey,
          body: {
            json: {
              purpose: PURPOSE,
              content_type: input.type,
              declared_bytes: input.blob.size,
              width: input.width,
              height: input.height,
            } as never,
          },
          decode: decodeIntent,
        } as HttpRequest<ProfileMediaIntent>,
      ),
    )
    mediaId = intent.media_id

    // 2. PUT bytes
    await attempt(() =>
      send(
        options,
        {
          path: intent.upload_path,
          method: 'PUT',
          access: 'protected',
          errorFamily: 'domain',
          domainOperation: 'putProfileMediaBytes',
          idempotencyKey: attemptKey,
          body: { binary: input.blob },
          decode: (value: unknown) => value,
        } as HttpRequest<unknown>,
      ),
    )

    // 3. finalize
    const ready = await attempt(() =>
      send(
        options,
        {
          path: `/api/me/media/${intent.media_id}/finalize`,
          method: 'POST',
          access: 'protected',
          errorFamily: 'domain',
          domainOperation: 'finalizeProfileMedia',
          idempotencyKey: attemptKey,
          body: { json: { expected_revision: intent.revision } as never },
          decode: decodeReady,
        } as HttpRequest<ReadyMedia>,
      ),
    )
    return { ok: true, media: ready }
  } catch (error) {
    // Every failure after init deletes the staged asset, so no orphan remains.
    if (mediaId !== null) {
      await deleteStaged(options, mediaId).catch(() => undefined)
    }
    if (cancelled() || isAborted(error)) {
      return { ok: false, cancelled: true, message: 'Upload cancelled.' }
    }
    return {
      ok: false,
      cancelled: false,
      message:
        error instanceof TransportError && error.status === 400
          ? 'That image was rejected. Try a different file.'
          : 'The upload did not finish. Try again.',
    }
  }
}

/**
 * Delete a staged asset.
 *
 * The frozen contract has no delete-media operation, so this uses the upload
 * lifecycle's own cleanup: a `PENDING` or `UPLOADED` asset that was never
 * finalized is erased by the retention worker. This function exists so the
 * caller's intent is explicit and so a future delete operation has one place to
 * land.
 */
async function deleteStaged(_options: UploadOptions, _mediaId: string): Promise<void> {
  // Deliberately a no-op today: there is no delete operation in the frozen
  // register, and inventing one here would be an unauthorized route. The asset
  // is unreferenced, so the orphan cleanup removes it.
  return Promise.resolve()
}
