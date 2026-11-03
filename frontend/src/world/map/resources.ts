import { MapError, requireMap, sha256 } from './json'
import { limits, requireProvided, validateManifests, type ReleasePin } from './manifest'
import { validateTiled } from './tiled'
export interface Resource { readonly path: string; readonly sha256: string; readonly kind: 'json' | 'png' }
export interface AssetResources { read(resource: Resource, signal: AbortSignal): Promise<Uint8Array<ArrayBuffer>> }
export function abortable<T>(work: Promise<T>, signal: AbortSignal, timeoutMs: number = limits.deadlineMs): Promise<T> {
  return new Promise((resolve, reject) => {
    let done = false
    const finish = (result: T | undefined, error?: unknown) => {
      if (done) return; done = true; clearTimeout(timer); signal.removeEventListener('abort', abort)
      if (error) reject(error); else resolve(result as T)
    }
    const abort = () => finish(undefined, new MapError('cancelled'))
    const timer = setTimeout(() => finish(undefined, new MapError('timeout')), timeoutMs)
    signal.addEventListener('abort', abort, { once: true })
    if (signal.aborted) abort()
    work.then(value => finish(value), error => finish(undefined, error))
  })
}
export function createAssetResources(options: { origin: string; allow: readonly Resource[]; fetch?: typeof fetch }): AssetResources {
  const origin = new URL(options.origin).origin
  requireMap(origin === options.origin && /^https?:$/.test(new URL(origin).protocol))
  const allowed = new Map<string, Resource>()
  for (const r of options.allow) {
    requireMap(/^world\/(?:[A-Za-z0-9_-]+\/)*[A-Za-z0-9_-]+\.(json|png)$/.test(r.path)
      && r.path.endsWith(r.kind === 'png' ? '.png' : '.json') && /^sha256:[0-9a-f]{64}$/.test(r.sha256) && !allowed.has(r.path))
    allowed.set(r.path, Object.freeze({ ...r }))
  }
  const fetchResource = options.fetch ?? globalThis.fetch.bind(globalThis)
  return { async read(resource, signal) {
    const approved = allowed.get(resource.path)
    requireMap(approved && approved.sha256 === resource.sha256 && approved.kind === resource.kind)
    if (signal.aborted) throw new MapError('cancelled')
    const controller = new AbortController(), abort = () => controller.abort()
    signal.addEventListener('abort', abort, { once: true })
    const max = resource.kind === 'png' ? limits.pngBytes : limits.tiledBytes, url = `${origin}/${resource.path}`
    const operation = async () => {
      const response = await fetchResource(url, { method: 'GET', credentials: 'omit', mode: 'same-origin', cache: 'force-cache', redirect: 'error', referrerPolicy: 'no-referrer', signal: controller.signal })
      if (!response.ok) throw new MapError('load-failed')
      requireMap(!response.redirected && response.url === url && response.body)
      const type = response.headers.get('Content-Type')?.split(';')[0]?.trim()
      requireMap(type === (resource.kind === 'png' ? 'image/png' : 'application/json'))
      const length = response.headers.get('Content-Length')
      if (length !== null) requireMap(/^\d+$/.test(length) && Number(length) <= max)
      const reader = response.body.getReader(), chunks: Uint8Array[] = []; let size = 0
      try {
        while (true) {
          const chunk = await reader.read(); if (chunk.done) break
          size += chunk.value.byteLength; requireMap(size <= max); chunks.push(chunk.value)
        }
      } finally { await reader.cancel().catch(() => {}); reader.releaseLock() }
      requireMap(size > 0)
      const bytes = new Uint8Array(size); let offset = 0
      for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length }
      requireMap(await sha256(bytes) === resource.sha256)
      if (controller.signal.aborted) throw new MapError('cancelled')
      return bytes
    }
    try { return await abortable(operation(), signal) }
    finally { signal.removeEventListener('abort', abort); controller.abort() }
  } }
}
export function pngDimensions(bytes: Uint8Array<ArrayBuffer>, width: number, height: number) {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10]
  requireMap(bytes.length >= 33 && bytes.length <= limits.pngBytes && signature.every((n, i) => bytes[i] === n))
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  requireMap(view.getUint32(8) === 13 && String.fromCharCode(...bytes.slice(12, 16)) === 'IHDR'
    && view.getUint32(16) === width && view.getUint32(20) === height && width > 0 && height > 0
    && width <= limits.textureSide && height <= limits.textureSide && width * height <= limits.texturePixels)
}
export interface DecodedImage { image: HTMLImageElement; dispose(): void }
export async function decodeImage(bytes: Uint8Array<ArrayBuffer>, signal: AbortSignal): Promise<DecodedImage> {
  if (signal.aborted) throw new MapError('cancelled')
  const image = new Image(), url = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }))
  let disposed = false
  const dispose = () => { if (!disposed) { disposed = true; image.src = ''; URL.revokeObjectURL(url) } }
  signal.addEventListener('abort', dispose, { once: true })
  try {
    image.src = url; await abortable(image.decode(), signal)
    if (signal.aborted) throw new MapError('cancelled')
    return { image, dispose }
  } catch (error) { dispose(); throw error }
  finally { signal.removeEventListener('abort', dispose) }
}
export async function loadMap(input: { mapText: string; themeText: string; pin: ReleasePin; resources: AssetResources;
  decode?: typeof decodeImage }, signal: AbortSignal) {
  const metadata = await validateManifests(input.mapText, input.themeText, input.pin)
  if (signal.aborted) throw new MapError('cancelled')
  const { map, theme } = metadata
  requireProvided(map, theme)
  const tiledResource: Resource = { path: map.tilemap.path!, sha256: map.tilemap.sha256!, kind: 'json' }
  const textureResource: Resource = { path: theme.texture.path!, sha256: theme.texture.sha256!, kind: 'png' }
  const tiled = await input.resources.read(tiledResource, signal)
  requireMap(tiled.length <= limits.tiledBytes && await sha256(tiled) === tiledResource.sha256)
  validateTiled(new TextDecoder('utf-8', { fatal: true }).decode(tiled), map.geometry, theme.texture)
  const bytes = await input.resources.read(textureResource, signal)
  requireMap(await sha256(bytes) === textureResource.sha256)
  pngDimensions(bytes, theme.texture.width_px, theme.texture.height_px)
  const decoded = await (input.decode ?? decodeImage)(bytes, signal)
  try {
    requireMap(decoded.image.naturalWidth === theme.texture.width_px && decoded.image.naturalHeight === theme.texture.height_px)
    if (signal.aborted) throw new MapError('cancelled')
    return { ...metadata, image: decoded.image, dispose: decoded.dispose }
  } catch (error) { decoded.dispose(); throw error }
}
export type LoadedMap = Awaited<ReturnType<typeof loadMap>>
