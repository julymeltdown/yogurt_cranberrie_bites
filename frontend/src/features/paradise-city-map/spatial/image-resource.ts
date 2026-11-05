import { requireSiteplan, SiteplanError, type PhysicalVenueSnapshot } from './contract'
import { createSiteplanModel, type SiteplanModel } from './model'
import { freezeSiteplanRegistration, type ResourcePin, type SiteplanRegistration } from './registration'

export interface DecodedSiteplanImage {
  readonly source: CanvasImageSource; readonly naturalWidth: number; readonly naturalHeight: number
  dispose(): void
}
export interface LoadedSiteplan {
  readonly model: SiteplanModel; readonly snapshot: PhysicalVenueSnapshot; readonly image: DecodedSiteplanImage
  readonly producer: SiteplanRegistration['producer']
  dispose(): void
}
export interface SiteplanLoaderOptions {
  readonly origin: string; readonly fetch?: typeof fetch
  readonly decode?: (bytes: Uint8Array, signal: AbortSignal) => Promise<DecodedSiteplanImage>
}
export interface SiteplanLoader { load(signal?: AbortSignal): Promise<LoadedSiteplan>; dispose(): void }
export async function siteplanSHA256(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new Uint8Array(bytes).buffer)
  return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('')
}
function abortable<T>(promise: Promise<T>, signal: AbortSignal, late: (value: T) => void): Promise<T> {
  return new Promise((resolve, reject) => {
    let settled = false
    const abort = () => { if (!settled) { settled = true; reject(new SiteplanError('ABORTED')) } }
    if (signal.aborted) abort()
    else signal.addEventListener('abort', abort, { once: true })
    promise.then(value => {
      signal.removeEventListener('abort', abort)
      if (settled || signal.aborted) { late(value); abort(); return }
      settled = true; resolve(value)
    }, error => { signal.removeEventListener('abort', abort); if (!settled) { settled = true; reject(error) } })
  })
}
async function readResource(pin: ResourcePin, origin: URL, request: typeof fetch, signal: AbortSignal): Promise<Uint8Array> {
  const url = new URL(pin.path, origin)
  requireSiteplan(url.origin === origin.origin && url.pathname === '/' + pin.path && !url.search && !url.hash)
  const response = await abortable(request(url.href, { signal, credentials: 'same-origin', redirect: 'error' }), signal, value => { void value.body?.cancel().catch(() => {}) })
  requireSiteplan(response.ok && response.body, 'NETWORK')
  const length = response.headers.get('content-length')
  requireSiteplan(length === null || Number(length) === pin.bytes, 'INVALID_CONTRACT')
  const reader = response.body.getReader(), parts: Uint8Array[] = []
  let total = 0, complete = false
  try {
    while (true) {
      const chunk = await abortable(reader.read(), signal, () => {})
      if (chunk.done) break
      total += chunk.value.byteLength
      requireSiteplan(total <= pin.bytes, 'INVALID_CONTRACT')
      parts.push(chunk.value)
    }
    requireSiteplan(total === pin.bytes, 'INVALID_CONTRACT')
    const bytes = new Uint8Array(total); let offset = 0
    for (const part of parts) { bytes.set(part, offset); offset += part.byteLength }
    requireSiteplan(await siteplanSHA256(bytes) === pin.sha256, 'HASH_MISMATCH')
    complete = true; return bytes
  } finally {
    if (!complete) await reader.cancel().catch(() => {})
    reader.releaseLock()
  }
}
export function decodeSiteplanImage(bytes: Uint8Array, signal: AbortSignal): Promise<DecodedSiteplanImage> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) { reject(new SiteplanError('ABORTED')); return }
    const image = new Image(), url = URL.createObjectURL(new Blob([new Uint8Array(bytes).buffer], { type: 'image/jpeg' }))
    let released = false
    const dispose = () => { if (!released) { released = true; image.src = ''; URL.revokeObjectURL(url) } }
    const clean = () => { image.onload = null; image.onerror = null; signal.removeEventListener('abort', abort) }
    const abort = () => { clean(); dispose(); reject(new SiteplanError('ABORTED')) }
    image.onload = () => { clean(); if (signal.aborted) { dispose(); reject(new SiteplanError('ABORTED')); return }; resolve(Object.freeze({ source: image, naturalWidth: image.naturalWidth, naturalHeight: image.naturalHeight, dispose })) }
    image.onerror = () => { clean(); dispose(); reject(new SiteplanError('NETWORK')) }
    signal.addEventListener('abort', abort, { once: true }); image.src = url
  })
}
/** One active public illustration lease; no Auth, GPS, friend or World connection. */
export function createSiteplanLoader(input: SiteplanRegistration, options: SiteplanLoaderOptions): SiteplanLoader {
  const registration = freezeSiteplanRegistration(input), origin = new URL(options.origin)
  requireSiteplan((origin.protocol === 'https:' || origin.protocol === 'http:') && !origin.username && !origin.password)
  origin.pathname = '/'; origin.search = ''; origin.hash = ''
  const request = options.fetch ?? fetch, decode = options.decode ?? decodeSiteplanImage
  let generation = 0, closed = false, pending: AbortController | null = null, current: LoadedSiteplan | null = null
  return Object.freeze({
    async load(external?: AbortSignal): Promise<LoadedSiteplan> {
      requireSiteplan(!closed, 'UNAVAILABLE')
      pending?.abort(); current?.dispose(); current = null
      const controller = new AbortController(), ticket = ++generation; pending = controller
      const signal = AbortSignal.any([controller.signal, ...(external ? [external] : [])])
      const timeout = setTimeout(() => controller.abort(), 15000)
      const check = () => { requireSiteplan(!closed && ticket === generation, 'STALE'); requireSiteplan(!signal.aborted, 'ABORTED') }
      let decoded: DecodedSiteplanImage | null = null
      try {
        check()
        const bytes = await readResource(registration.manifest, origin, request, signal); check()
        let document: unknown
        try { document = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)) } catch { throw new SiteplanError('INVALID_CONTRACT') }
        const model = createSiteplanModel(document, registration); check()
        const imageBytes = await readResource(registration.image, origin, request, signal); check()
        decoded = await abortable(decode(imageBytes, signal), signal, value => value.dispose()); check()
        requireSiteplan(decoded.naturalWidth === registration.image.width && decoded.naturalHeight === registration.image.height, 'DIMENSION_MISMATCH')
        const image = decoded; let released = false
        const dispose = () => { if (!released) { released = true; signal.removeEventListener('abort', dispose); image.dispose(); if (current === lease) current = null } }
        const lease: LoadedSiteplan = Object.freeze({ model, snapshot: model.snapshot, image, producer: registration.producer, dispose })
        signal.addEventListener('abort', dispose, { once: true }); current = lease; check(); return lease
      } catch (error) {
        const wasAborted = signal.aborted
        controller.abort()
        decoded?.dispose()
        if (closed || ticket !== generation) throw new SiteplanError('STALE')
        if (wasAborted) throw new SiteplanError('ABORTED')
        if (error instanceof SiteplanError) throw error
        throw new SiteplanError('NETWORK')
      } finally {
        clearTimeout(timeout)
      }
    },
    dispose() { if (!closed) { closed = true; generation++; pending?.abort(); pending = null; current?.dispose(); current = null } },
  })
}
