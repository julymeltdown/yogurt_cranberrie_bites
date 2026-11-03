import { characterSourcePack, characterArtSources } from '../source-catalog'
import { createAssetResources, pngDimensions, type AssetResources, type Resource } from '../../../world/map/resources'

/** Public illustration metadata. These IDs are never AvatarConfig item IDs. */
export interface StaticCharacter {
  readonly id: string
  readonly label: string
  readonly source_id: string
  readonly source_sha256: string
  readonly frame: { readonly x: number; readonly y: 0; readonly width_px: 64; readonly height_px: 64 }
  readonly anchor: { readonly x: 32; readonly y: 60 }
}
export interface StaticImage extends Resource { readonly kind: 'png'; readonly width_px: number; readonly height_px: number }
export interface SelectedStaticRelease {
  readonly format: 'mcr-selected-static-v1'
  readonly pose: 'front_idle_only'
  readonly source_pack_id: string
  readonly source_pack_revision: number
  readonly reviewed_by: string
  readonly reviewed_at: string
  readonly atlas: StaticImage
  readonly characters: readonly StaticCharacter[]
}
const object = (value: unknown): Record<string, unknown> => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Static artwork unavailable')
  return value as Record<string, unknown>
}
const ensure: (value: unknown) => asserts value = value => { if (!value) throw new Error('Static artwork unavailable') }
function image(value: unknown): StaticImage {
  const item = object(value)
  ensure(typeof item.path === 'string' && /^world\/avatars\/derived-v1\/[a-zA-Z0-9_-]+\.png$/.test(item.path)
    && typeof item.sha256 === 'string' && /^sha256:[0-9a-f]{64}$/.test(item.sha256)
    && Number.isInteger(item.width_px) && Number.isInteger(item.height_px)
    && Number(item.width_px) > 0 && Number(item.height_px) > 0
    && item.width_px === 384 && item.height_px === 64)
  return Object.freeze({ path: item.path, sha256: item.sha256, kind: 'png', width_px: Number(item.width_px), height_px: Number(item.height_px) })
}
/** Deliberately separate from the layered/directional C06 contract. No generated fallback. */
export function validateSelectedStaticRelease(value: unknown): SelectedStaticRelease {
  const release = object(value)
  ensure(release.format === 'mcr-selected-static-v1' && release.pose === 'front_idle_only'
    && release.source_pack_id === characterSourcePack.source_pack_id && release.source_pack_revision === characterSourcePack.revision
    && typeof release.reviewed_by === 'string' && release.reviewed_by.trim().length > 0 && release.reviewed_by.length <= 160
    && typeof release.reviewed_at === 'string' && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(release.reviewed_at)
    && Number.isFinite(Date.parse(release.reviewed_at)) && Array.isArray(release.characters) && release.characters.length === 6)
  const characters = release.characters.map((value, index) => {
    const item = object(value), anchor = object(item.anchor), frame = object(item.frame)
    ensure(item.id === `photo01-front-0${index + 1}` && typeof item.label === 'string' && item.label.trim().length > 0 && item.label.length <= 120
      && item.source_id === 'photo_01' && typeof item.source_sha256 === 'string'
      && characterArtSources.some(source => source.source_id === item.source_id && source.sha256 === item.source_sha256)
      && anchor.x === 32 && anchor.y === 60 && frame.x === index * 64 && frame.y === 0 && frame.width_px === 64 && frame.height_px === 64)
    return Object.freeze({ id: item.id, label: item.label, source_id: item.source_id, source_sha256: item.source_sha256,
      frame: Object.freeze({ x: index * 64, y: 0 as const, width_px: 64 as const, height_px: 64 as const }), anchor: Object.freeze({ x: 32 as const, y: 60 as const }) })
  })
  return Object.freeze({ format: 'mcr-selected-static-v1', pose: 'front_idle_only', source_pack_id: release.source_pack_id,
    source_pack_revision: release.source_pack_revision, reviewed_by: release.reviewed_by, reviewed_at: release.reviewed_at,
    atlas: image(release.atlas), characters: Object.freeze(characters) })
}

export async function readSelectedStaticRelease(signal: AbortSignal, fetcher: typeof fetch = fetch): Promise<SelectedStaticRelease> {
  const response = await fetcher('/world/avatars/derived-v1/static-release.json', {
    method: 'GET', credentials: 'omit', mode: 'same-origin', redirect: 'error', referrerPolicy: 'no-referrer', signal,
  })
  ensure(response.ok && !response.redirected && response.headers.get('content-type')?.split(';')[0]?.trim() === 'application/json')
  const reader = response.body?.getReader(); ensure(reader)
  const chunks: Uint8Array[] = []; let length = 0
  try {
    while (true) { const result = await reader.read(); if (result.done) break; length += result.value.length; ensure(length <= 32768); chunks.push(result.value) }
  } finally { await reader.cancel().catch(() => {}); reader.releaseLock() }
  const bytes = new Uint8Array(length); let offset = 0
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length }
  ensure(!signal.aborted)
  return validateSelectedStaticRelease(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)))
}

export function staticResources(release: SelectedStaticRelease, origin: string): AssetResources {
  return createAssetResources({ origin, allow: [release.atlas] })
}
export async function readStaticImage(image: StaticImage, resources: AssetResources, signal: AbortSignal): Promise<Uint8Array<ArrayBuffer>> {
  const bytes = await resources.read(image, signal)
  pngDimensions(bytes, image.width_px, image.height_px)
  ensure(!signal.aborted)
  return bytes
}

/** Public immutable art only: one bounded release/atlas fetch across screen and era changes. */
let cached: Promise<{ release: SelectedStaticRelease; bytes: Uint8Array<ArrayBuffer> }> | undefined
export function loadSelectedStaticArtwork() {
  if (!cached) cached = (async () => {
    const controller = new AbortController(), timer = setTimeout(() => controller.abort(), 15000)
    try {
      const release = await readSelectedStaticRelease(controller.signal)
      const bytes = await readStaticImage(release.atlas, staticResources(release, location.origin), controller.signal)
      return { release, bytes }
    } finally { clearTimeout(timer) }
  })().catch(error => { cached = undefined; throw error })
  return cached
}
