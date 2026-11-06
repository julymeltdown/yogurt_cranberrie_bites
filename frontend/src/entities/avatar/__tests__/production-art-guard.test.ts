// @vitest-environment jsdom
import { createHash } from 'node:crypto'
import { readdir, readFile } from 'node:fs/promises'
import { URL as FileURL, fileURLToPath, pathToFileURL } from 'node:url'
import { afterEach, expect, it, vi } from 'vitest'
import c06 from '../../../../../contracts/fixtures/assets/avatar-manifest.json'
import reviewedSchema from '../../../../../contracts/assets/v2/reviewed/avatar-manifest.schema.json'
import pack from '../character-source-pack.json'
import { characterArtSources, getCharacterSourceUrl } from '../source-catalog'
import { MOTION_ART, loadMotionArtwork, validateMotionArt } from '../runtime/motion-adoption/art'
import { decodeReviewedMotionManifest, REVIEWED_MOTION, type ReviewedMotionManifest } from '../runtime/motion-adoption/reviewed-manifest'
import { AvatarError, type AvatarManifest } from '../runtime/contract'
import { validateBinding, type ReviewedAvatarBinding } from '../runtime/resources'
import { contentHash } from '../../../world/map/json'

interface MotionFrame { readonly preset: string; readonly attribution: string }
interface MotionItem { readonly key: string; readonly kind: string; readonly directions: readonly { readonly frames: readonly number[] }[] }
interface CandidateDescriptor {
  readonly productionEligible: boolean
  readonly sourcePack: { readonly id: string; readonly revision: number }
  readonly atlas: { readonly width: number; readonly height: number; readonly file: string; readonly sha256: string; readonly bytes: number }
  readonly physicalFrames: number
  readonly presets: { readonly frames: readonly MotionFrame[]; readonly directions: readonly string[]; readonly phases: readonly string[] }
  readonly wardrobe: { readonly items: readonly MotionItem[] }
}
type ReviewedManifest = ReviewedMotionManifest & {
  readonly source: ReviewedMotionManifest['source'] & {
    readonly originals: readonly { readonly source_id: string; readonly resource: { readonly path: string; readonly sha256: string; readonly bytes: number } }[]
  }
}

const repository = fileURLToPath(new FileURL('../../../../../', import.meta.url)).replace(/\/$/, '')
const sourceRoot = fileURLToPath(new FileURL('../../../', import.meta.url)).replace(/\/$/, '')
const publicRoot = `${repository}/frontend/public`
const asset = (path: string) => new FileURL(path, pathToFileURL(`${repository}/`))
const bytes = async (path: string) => new Uint8Array(await readFile(asset(path)))
const sha = (value: Uint8Array) => createHash('sha256').update(value).digest('hex')
const pngDimensions = (value: Uint8Array) => {
  const view = new DataView(value.buffer, value.byteOffset, value.byteLength)
  return [view.getUint32(16), view.getUint32(20)]
}
const jpegDimensions = (value: Uint8Array): [number, number] => {
  let offset = 2
  while (offset < value.length) {
    if (value[offset] !== 0xff) { offset++; continue }
    const marker = value[offset + 1]!
    if (marker === 0xd8 || marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) { offset += 2; continue }
    const length = (value[offset + 2]! << 8) | value[offset + 3]!
    if ([0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf].includes(marker)) {
      return [((value[offset + 7]! << 8) | value[offset + 8]!), ((value[offset + 5]! << 8) | value[offset + 6]!)]
    }
    offset += 2 + length
  }
  throw new Error('JPEG dimensions unavailable')
}
async function walk(path: string): Promise<string[]> {
  const entries = await readdir(path, { withFileTypes: true }), files: string[] = []
  for (const entry of entries) {
    const child = `${path}/${entry.name}`
    if (entry.isDirectory()) files.push(...await walk(child))
    else files.push(child)
  }
  return files
}
const selected = new Map(pack.files.map(file => [file.source_id, file]))
const selectedIds = ['photo_01', 'photo_02', 'photo_04', 'photo_05', 'photo_06', 'photo_07', 'photo_08', 'photo_09', 'photo_10',
  'photo_11', 'photo_12', 'photo_13', 'photo_14', 'photo_15', 'photo_16', 'photo_17', 'photo_18', 'photo_19', 'photo_20', 'photo_21']

afterEach(() => vi.unstubAllGlobals())

it('retains every selected source sheet byte for byte and refuses archived or unknown IDs', async () => {
  expect(pack.source_pack_id).toBe('mcr_user_characters_20261103')
  expect(pack.revision).toBe(3)
  expect(pack.files.map(file => file.source_id)).toEqual(selectedIds)
  expect(pack.files).toHaveLength(20)
  expect(pack.archived_files.map(file => file.source_id)).toEqual(['photo_03'])
  expect(pack.allow_legacy_character_fallback).toBe(false)
  expect(characterArtSources).toEqual(pack.files)
  for (const file of pack.files) {
    const value = await bytes(`frontend/public${pack.base_url}${file.file}`)
    expect(value.byteLength).toBe(file.byte_length)
    expect(sha(value)).toBe(file.sha256)
    expect(jpegDimensions(value)).toEqual([file.width_px, file.height_px])
    expect(getCharacterSourceUrl(file.source_id)).toBe(`${pack.base_url}${file.file}`)
    expect(file.duplicate_of).toBeNull()
  }
  const archived = pack.archived_files[0]!
  const archivedBytes = await bytes(`frontend/public${pack.base_url}${archived.file}`)
  expect(sha(archivedBytes)).toBe(archived.sha256)
  expect(jpegDimensions(archivedBytes)).toEqual([archived.width_px, archived.height_px])
  expect(() => getCharacterSourceUrl('photo_03')).toThrow(RangeError)
  expect(() => getCharacterSourceUrl('legacy_character')).toThrow(RangeError)
})

it('pins the produced atlas and reviewed manifest to the selected source pack, dimensions, frames and IDs', async () => {
  const descriptorBytes = await bytes(`frontend/public/${MOTION_ART.descriptor.path}`)
  expect(descriptorBytes.byteLength).toBe(MOTION_ART.descriptor.bytes)
  expect(`sha256:${sha(descriptorBytes)}`).toBe(MOTION_ART.descriptor.sha256)
  const art = await validateMotionArt(descriptorBytes), descriptor = JSON.parse(new TextDecoder().decode(descriptorBytes)) as CandidateDescriptor
  expect(art.descriptor.productionEligible).toBe(false)
  expect(descriptor.productionEligible).toBe(false)
  expect(descriptor.sourcePack).toEqual({ id: pack.source_pack_id, revision: pack.revision })
  expect(descriptor.atlas).toEqual({ width: 1280, height: 1856, file: MOTION_ART.atlas.path.split('/').at(-1),
    sha256: MOTION_ART.atlas.sha256.replace('sha256:', ''), bytes: MOTION_ART.atlas.bytes })
  expect(descriptor.physicalFrames).toBe(576)
  expect(descriptor.presets.frames).toHaveLength(120)
  expect(new Set(descriptor.presets.frames.map(frame => frame.preset)).size).toBe(6)
  for (const preset of new Set(descriptor.presets.frames.map(frame => frame.preset))) {
    expect(descriptor.presets.frames.filter(frame => frame.preset === preset)).toHaveLength(20)
  }
  expect(descriptor.presets.directions).toEqual(['front', 'left', 'right', 'rear'])
  expect(descriptor.presets.phases).toHaveLength(5)
  expect(descriptor.presets.frames.filter(frame => frame.attribution === 'SUPPLIED_FRONT_PIXELS_SOURCE_ARTICULATION')).toHaveLength(30)
  expect(descriptor.presets.frames.filter(frame => frame.attribution === 'AUTHORED_RECONSTRUCTION_WITH_ARTICULATED_SOURCE')).toHaveLength(90)
  expect(descriptor.wardrobe.items).toHaveLength(28)
  expect(Object.fromEntries(['body', 'eyes', 'hair', 'outfit', 'accessory'].map(kind =>
    [kind, descriptor.wardrobe.items.filter(item => item.kind === kind).length]))).toEqual({ body: 8, eyes: 4, hair: 6, outfit: 6, accessory: 4 })
  expect(new Set(descriptor.wardrobe.items.map(item => item.key)).size).toBe(28)
  expect(descriptor.wardrobe.items.reduce((total, item) => total + item.directions.reduce((sum, direction) => sum + direction.frames.length, 0), 0)).toBe(560)

  const atlasBytes = await bytes(`frontend/public/${MOTION_ART.atlas.path}`)
  expect(atlasBytes.byteLength).toBe(MOTION_ART.atlas.bytes)
  expect(`sha256:${sha(atlasBytes)}`).toBe(MOTION_ART.atlas.sha256)
  expect(pngDimensions(atlasBytes)).toEqual([1280, 1856])

  const manifestBytes = await bytes('backend/releases/selected-character-motion-v2/reviewed/manifest.json')
  expect(manifestBytes.byteLength).toBe(REVIEWED_MOTION.manifest.bytes)
  expect(`sha256:${sha(manifestBytes)}`).toBe(REVIEWED_MOTION.manifest.sha256)
  const manifest = JSON.parse(new TextDecoder().decode(manifestBytes)) as ReviewedManifest
  expect(manifest).toEqual(reviewedSchema.$defs.ReviewedMotionAvatarManifest.const)
  expect(manifest.content_hash).toBe(REVIEWED_MOTION.contentHash)
  expect(await contentHash(manifest as unknown as Record<string, unknown>)).toBe(manifest.content_hash)
  expect(manifest).toMatchObject({ manifest_id: 'mcr_selected_motion_reviewed_20261104', manifest_version: 2,
    representation: 'reviewed_selected_source_motion', production_eligible: true, qualification: 'VISUALLY_REVIEWED', fallback: 'NONE',
    source: { pack_id: pack.source_pack_id, revision: pack.revision } })
  expect(manifest.source.descriptor).toEqual({ path: `frontend/public/${MOTION_ART.descriptor.path}`,
    sha256: MOTION_ART.descriptor.sha256.replace('sha256:', ''), bytes: MOTION_ART.descriptor.bytes })
  expect(manifest.atlas).toEqual({ path: `frontend/public/${MOTION_ART.atlas.path}`, sha256: MOTION_ART.atlas.sha256.replace('sha256:', ''),
    bytes: MOTION_ART.atlas.bytes, width: 1280, height: 1856 })
  expect(manifest.presets).toHaveLength(6)
  expect(manifest.items).toHaveLength(28)
  expect(manifest.source.originals).toHaveLength(20)
  for (const original of manifest.source.originals) {
    const file = selected.get(original.source_id)
    expect(file).toBeDefined()
    expect(original.resource).toEqual({ path: `frontend/public${pack.base_url}${file!.file}`, sha256: file!.sha256, bytes: file!.byte_length })
  }
  expect(manifest.source.originals.map(original => original.source_id)).toEqual(selectedIds)
  expect(manifest.source.originals.some(original => original.source_id === 'photo_03')).toBe(false)
  expect(manifest.review).toEqual({ path: 'docs/mcr/contracts/avatar-motion-v2/review/visual-review.json',
    sha256: REVIEWED_MOTION.review.sha256.replace('sha256:', ''), bytes: REVIEWED_MOTION.review.bytes })
  expect(manifest.candidate_manifest).toEqual({ path: 'backend/releases/selected-character-motion-v2/manifest.json',
    sha256: '9a555f726a9e421f14ed876117be87bb1bb8fa128ac401ab38f8927760776fd6', bytes: 95128 })
  expect(manifest.default_preset_id).toBe(manifest.presets.find(preset => preset.config.representation === 'motion_preset')?.preset_id)
  const stage = await decodeReviewedMotionManifest(manifest, { manifest_id: manifest.manifest_id, manifest_version: 2, content_hash: manifest.content_hash })
  expect(stage.qualification).toBe('STAGED_NOT_ADMITTED')
})

it('cannot admit the C06 synthetic fixture as product character art', () => {
  expect(c06.provenance.source_kind).toBe('ORIGINAL_FIXTURE')
  expect(c06.provenance.review_state).toBe('UNREVIEWED')
  expect(c06.atlas.texture).toEqual({ image_state: 'NOT_PROVIDED', path: null, sha256: null, width_px: 320, height_px: 1280 })
  const forged = {
    sourcePackId: pack.source_pack_id,
    sourcePackRevision: pack.revision,
    manifestHash: c06.content_hash,
    texturePath: null,
    textureHash: null,
    sources: pack.files.map(file => ({ sourceId: file.source_id, sha256: file.sha256 })),
    reviewedBy: 'synthetic fixture caller',
    reviewedAt: '2026-11-06T00:00:00Z',
    bakedSkinToneId: 'tone_fixture',
  } as unknown as ReviewedAvatarBinding
  expect(() => validateBinding(c06 as unknown as AvatarManifest, forged)).toThrow(AvatarError)
  expect(() => validateBinding(c06 as unknown as AvatarManifest, undefined)).toThrow(AvatarError)
})

it('keeps legacy, placeholder and synthetic art unreachable from production paths and public assets', async () => {
  const production = (await walk(sourceRoot)).filter(file => /\.(?:ts|tsx)$/.test(file))
    .filter(file => !file.includes('/__tests__/') && !file.includes('/generated/') && !/\.test\.(?:ts|tsx)$/.test(file))
  let references = 0
  for (const file of production) {
    const text = await readFile(file, 'utf8'), path = file.slice(`${sourceRoot}/`.length)
    for (const match of text.matchAll(/world\/avatars\/[A-Za-z0-9_./-]+/g)) {
      const reference = match[0]!
      references++
      expect(reference.startsWith('world/avatars/derived-v1/') || reference.startsWith('world/avatars/supplied-v1/'),
        `${path} references an unapproved avatar family: ${reference}`).toBe(true)
      expect(reference).not.toContain('photo-03')
    }
    if (text.includes('contracts/fixtures/assets/avatar-manifest.json')) {
      expect(text.trimStart().startsWith('import type'), `${path} must not import the synthetic C06 fixture as a value`).toBe(true)
    }
  }
  expect(references).toBeGreaterThan(0)

  const avatarRoot = `${publicRoot}/world/avatars`, top = (await readdir(avatarRoot, { withFileTypes: true })).map(entry => entry.name).sort()
  expect(top).toEqual(['derived-v1', 'supplied-v1'])
  const known = new Set([...pack.files, ...pack.archived_files].map(file => `supplied-v1/${file.file}`))
  for (const file of await walk(avatarRoot)) {
    const path = file.slice(`${avatarRoot}/`.length)
    expect(path.startsWith('supplied-v1/') || path.startsWith('derived-v1/'), `unapproved avatar asset: ${path}`).toBe(true)
    if (path.startsWith('supplied-v1/')) expect(known.has(path), `unknown supplied asset: ${path}`).toBe(true)
    expect(path).not.toMatch(/legacy|placeholder|fallback|old-avatar|default-avatar/i)
  }
})

it('loads the reviewed atlas from the exact public asset path and verifies its hash', async () => {
  const resources = new Map<string, Uint8Array<ArrayBuffer>>()
  for (const resource of [MOTION_ART.descriptor, MOTION_ART.atlas]) resources.set(resource.path, await bytes(`frontend/public/${resource.path}`))
  const requested: string[] = []
  const fetch = async (input: RequestInfo | URL) => {
    const url = String(input), path = new URL(url).pathname.slice(1), value = resources.get(path)
    if (!value) throw new Error(`Unexpected avatar request: ${url}`)
    requested.push(url)
    const response = new Response(value, { headers: { 'Content-Type': path.endsWith('.png') ? 'image/png' : 'application/json' } })
    Object.defineProperty(response, 'url', { value: url })
    return response
  }
  vi.stubGlobal('Image', class { src = ''; naturalWidth = 1280; naturalHeight = 1856; async decode() {} })
  vi.stubGlobal('URL', class extends URL { static createObjectURL() { return 'blob:production-art-guard' } static revokeObjectURL() {} })
  const loaded = await loadMotionArtwork({ origin: 'https://mcr.test', signal: new AbortController().signal, fetch })
  expect(requested).toEqual([`https://mcr.test/${MOTION_ART.descriptor.path}`, `https://mcr.test/${MOTION_ART.atlas.path}`])
  expect(loaded.art.descriptor.productionEligible).toBe(false)
  expect(loaded.image.naturalWidth).toBe(1280)
  expect(loaded.image.naturalHeight).toBe(1856)
  loaded.dispose()
})
