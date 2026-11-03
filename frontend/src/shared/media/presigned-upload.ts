import { checkAbort, ImageError, MAX_IMAGE_BYTES, IMAGE_TYPES } from './image-policy'
import type { ImageMime } from './types'
/** Independent future-contract helper. C04 does not return or select this mode. */
export async function uploadPresignedImage(options: {
  readonly url: string; readonly allowedOrigins: readonly string[]; readonly blob: Blob
  readonly signal: AbortSignal; readonly fetch?: typeof globalThis.fetch
}): Promise<void> {
  const { blob, signal } = options
  checkAbort(signal)
  const url = new URL(options.url)
  // Exact caller-qualified origin; forbid implicit relative URL, userinfo, fragment and redirects.
  if (url.protocol !== 'https:' || url.username || url.password || url.hash
    || !options.allowedOrigins.includes(url.origin) || options.allowedOrigins.some(origin => new URL(origin).origin !== origin)
    || !IMAGE_TYPES.includes(blob.type as ImageMime) || !blob.size || blob.size > MAX_IMAGE_BYTES) throw new ImageError('invalid-bytes')
  const lifetime = AbortSignal.any([signal, AbortSignal.timeout(15_000)])
  const response = await (options.fetch ?? globalThis.fetch)(url, {
    method: 'PUT', credentials: 'omit', cache: 'no-store', redirect: 'error', referrerPolicy: 'no-referrer',
    headers: { 'Content-Type': blob.type }, body: blob.slice(0, blob.size, blob.type), signal: lifetime,
  })
  void response.body?.cancel().catch(() => undefined)
  if (lifetime.aborted) throw new ImageError('aborted')
  if (!response.ok || response.redirected || response.type === 'opaqueredirect') throw new Error('Upload failed')
}
