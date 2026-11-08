// @vitest-environment jsdom
import { readFileSync } from 'node:fs'
import { URL as FileURL } from 'node:url'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMotionRuntime } from '../../../entities/emo-avatar/runtime'
import { MOTION_ART, validateMotionArt, type MotionArt } from '../../../entities/emo-avatar/art'
import { REVIEWED_MOTION } from '../../../entities/emo-avatar/reviewed-manifest'
import { PROFILE_AVATAR_PATH, createProfileAvatarPersistence } from '../api/persistence'
import { createProfileAvatarEditorOwner } from '../model/owner'
import { layered, pin, preset, setup, type Fixture } from './fixtures'

const cleanups: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.test' }))
afterEach(() => { cleanups.splice(0).reverse().forEach(fn => fn()); vi.unstubAllGlobals(); vi.restoreAllMocks() })

let art: MotionArt
async function loadArt() {
  art ??= await validateMotionArt(new Uint8Array(readFileSync(new FileURL(`../../../../public/${MOTION_ART.descriptor.path}`, import.meta.url))))
  return art
}
/** Source with comments removed: prose may name what the code deliberately does not do. */
function codeOnly(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, '').split('\n')
    .filter(line => !line.trimStart().startsWith('//')).join('\n')
}

const settle = async () => {
  // The catalog load awaits real hashing and image decode; poll the observable
  // state instead of guessing a microtask count.
  for (let i = 0; i < 200; i++) {
    await new Promise(resolve => setTimeout(resolve, 0))
    if (i % 10 === 0) await Promise.resolve()
  }
}

async function owner(f: Fixture) {
  const authority = await f.create()
  const runtime = createMotionRuntime({
    authority, scope: f.scope, privacy: f.auth.privacy, origin: 'https://mcr.test',
    load: async () => ({ art: await loadArt(), image: new Image(), dispose() {} }),
  })
  const value = createProfileAvatarEditorOwner({ auth: f.auth, scope: f.scope, authority, runtime })
  cleanups.push(value.dispose, runtime.dispose, authority.dispose)
  return value
}

// ---------------------------------------------------------------------------
// T-704: persistence points at /api/me/avatar and nothing else
// ---------------------------------------------------------------------------

it('reads and saves through /api/me/avatar with no event id in any request', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const authority = await f.create()
  cleanups.push(authority.dispose)
  const persistence = createProfileAvatarPersistence({ auth: f.auth, authority })
  cleanups.push(() => persistence.dispose())
  const controller = new AbortController()
  const read = await persistence.read(controller.signal)
  expect(read).toEqual({ revision: 1, config: preset(0) })
  const saved = await persistence.save(1, layered(), controller.signal)
  expect(saved).toEqual({ revision: 2, config: layered() })
  expect(f.calls.map(call => `${call.method} ${call.path}`)).toEqual([
    `GET ${PROFILE_AVATAR_PATH}`,
    `PUT ${PROFILE_AVATAR_PATH}`,
  ])
  // The hard constraint: no event id appears in any request, in the path or the body.
  const eventId = '00000000-0000-4000-8000-000000000701'
  for (const call of f.calls) {
    expect(call.path).not.toContain(eventId)
    expect(call.path).not.toContain('/events/')
    expect(JSON.stringify(call.body)).not.toContain(eventId)
    expect(JSON.stringify(call.body)).not.toContain('event_id')
    expect(JSON.stringify(call.body)).not.toContain('participant')
  }
  // The write body is exactly the frozen SetAvatarRequest.
  expect(Object.keys(f.calls[1]!.body as object).sort()).toEqual(['config', 'expected_revision'])
})

it('the persistence module has no event path, no event id and no fabricated authority', () => {
  const source = readFileSync(new FileURL('../api/persistence.ts', import.meta.url), 'utf8')
  // Only the explanatory comment may name the path this replaces; no code does.
  const code = codeOnly(source)
  expect(code).not.toContain('/api/concert/events/')
  expect(code).not.toContain('eventId')
  expect(code).not.toContain('event_id')
  expect(code).not.toContain('participant')
  expect(code).toContain("'/api/me/avatar'")
  // No test-only authority is reachable from production code.
  expect(code).not.toContain('TEST_ONLY')
  expect(code).not.toContain('createReviewedMotionFixtureAuthority')
})

// ---------------------------------------------------------------------------
// T-704: the editor saves and reloads the same configuration
// ---------------------------------------------------------------------------

it('saves a draft and reloads exactly the same configuration', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const editor = await owner(f)
  editor.start(); await settle()
  expect(editor.controller.read().phase).toBe('ready')
  expect(editor.controller.read().persisted).toEqual({ revision: 1, config: preset(0) })
  expect(editor.controller.selectPreset(editor.controller.read().catalog!.presets[3]!.id)).toBe(true)
  expect(await editor.controller.save()).toBe(true)
  expect(editor.controller.read().persisted).toEqual({ revision: 2, config: preset(3) })
  // A fresh owner for the same account reads the same configuration back.
  const second = await owner(f)
  second.start(); await settle()
  expect(second.controller.read().persisted).toEqual({ revision: 2, config: preset(3) })
  expect(second.controller.read().draft).toEqual(preset(3))
  expect(f.calls.map(call => `${call.method} ${call.path}`)).toEqual([
    `GET ${PROFILE_AVATAR_PATH}`, `PUT ${PROFILE_AVATAR_PATH}`, `GET ${PROFILE_AVATAR_PATH}`,
  ])
})

it('keeps the draft local until an explicit save', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const editor = await owner(f)
  editor.start(); await settle()
  editor.controller.selectPreset(editor.controller.read().catalog!.presets[2]!.id)
  expect(f.calls.filter(call => call.method === 'PUT')).toHaveLength(0)
  expect(f.server().config).toEqual(preset(0))
  editor.controller.reset()
  expect(editor.controller.read().draft).toEqual(preset(0))
})

it('reconciles a revision conflict without losing the draft', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const editor = await owner(f)
  editor.start(); await settle()
  editor.controller.selectPreset(editor.controller.read().catalog!.presets[4]!.id)
  f.mode('conflict')
  expect(await editor.controller.save()).toBe(false)
  expect(editor.controller.read()).toMatchObject({ notice: 'conflict', needsReconciliation: true })
  expect(editor.controller.read().draft).toEqual(preset(4))
  f.mode('ok')
  await editor.controller.reload()
  expect(editor.controller.read().candidate).toEqual(f.server())
  expect(editor.controller.resolveConflict('keep-draft')).toBe(true)
  expect(editor.controller.read().needsReconciliation).toBe(false)
})

it('a denied save keeps the draft and does not claim success', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const editor = await owner(f)
  editor.start(); await settle()
  editor.controller.selectPreset(editor.controller.read().catalog!.presets[1]!.id)
  f.mode('denied')
  expect(await editor.controller.save()).toBe(false)
  expect(editor.controller.read()).toMatchObject({ notice: 'save-failed', needsReconciliation: false })
  expect(editor.controller.read().draft).toEqual(preset(1))
})

// ---------------------------------------------------------------------------
// T-704: no fallback character art
// ---------------------------------------------------------------------------

it('allow_legacy_character_fallback stays false and the profile entity references no fallback path', () => {
  const pack = JSON.parse(readFileSync(new FileURL('../../../../src/entities/avatar/character-source-pack.json', import.meta.url), 'utf8')) as Record<string, unknown>
  expect(pack.allow_legacy_character_fallback).toBe(false)
  expect(pack.source_pack_id).toBe('mcr_user_characters_20261103')
  expect(pack.revision).toBe(3)
  // The profile entity and feature sources must not name a fallback, a legacy
  // character, or any substitute artwork.
  const entity = (name: string) => new FileURL(`../../../../src/entities/emo-avatar/${name}`, import.meta.url)
  const feature = (name: string) => new FileURL(`../${name}`, import.meta.url)
  const read = (url: URL) => { try { return readFileSync(url, 'utf8') } catch { return '' } }
  for (const [path, url] of [
    ['entities/emo-avatar/art', entity('art.ts')],
    ['entities/emo-avatar/compositor', entity('compositor.ts')],
    ['entities/emo-avatar/runtime', entity('runtime.ts')],
    ['entities/emo-avatar/reviewed-production', entity('reviewed-production.ts')],
    ['entities/emo-avatar/index', entity('index.ts')],
    ['api/persistence', feature('api/persistence.ts')],
    ['model/controller', feature('model/controller.ts')],
    ['model/owner', feature('model/owner.ts')],
    ['model/cache', feature('model/cache.ts')],
    ['ui/ProfileAvatarEditor', feature('ui/ProfileAvatarEditor.tsx')],
    ['ui/ProfileAvatarPanel', feature('ui/ProfileAvatarPanel.tsx')],
    ['ui/MotionView', feature('ui/MotionView.tsx')],
    ['ui/MotionOption', feature('ui/MotionOption.tsx')],
  ] as const) {
    const source = read(url)
    // The manifest's own `fallback: 'NONE'` declaration is the *proof* that no
    // fallback exists, so it is allowed; a fallback *path* or *artwork* is not.
    for (const forbidden of ['legacy_character', 'legacyCharacter', 'fallbackCharacter', 'fallback_character',
      'default-avatar', 'defaultAvatar', 'placeholder-avatar', 'placeholderAvatar', 'fallbackArt', 'fallback_art',
      'fallbackPath', 'fallback_path', 'fallbackUrl', 'fallback_url', 'fallbackSrc', 'fallback_src',
      'legacy_character_fallback', 'legacyFallback', 'legacy_fallback']) {
      expect(source, `${path} must not reference ${forbidden}`).not.toContain(forbidden)
    }
  }
})

it('refuses to render a character when the artwork cannot be verified, with no substitute', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const authority = await f.create()
  cleanups.push(authority.dispose)
  const runtime = createMotionRuntime({
    authority, scope: f.scope, privacy: f.auth.privacy, origin: 'https://mcr.test',
    load: async () => { throw new Error('artwork unavailable') },
  })
  cleanups.push(runtime.dispose)
  const editor = createProfileAvatarEditorOwner({ auth: f.auth, scope: f.scope, authority, runtime })
  cleanups.push(editor.dispose)
  editor.start(); await settle()
  expect(editor.controller.read()).toMatchObject({ phase: 'unavailable', notice: 'art-unavailable' })
  expect(editor.controller.read().catalog).toBeNull()
  expect(editor.controller.read().draft).toBeNull()
})

it('the pinned atlas path and hash contract is preserved', () => {
  // The same descriptor and atlas the concert editor pinned, unchanged.
  expect(MOTION_ART.descriptor).toEqual({
    path: 'world/avatars/derived-v1/motion-candidates-v1/combined-release-candidate.json',
    sha256: 'sha256:6cf3aad25ceba681f4c0306d1139109ee6c07508de617ecf1c0b08b1d43fb950', bytes: 42925, kind: 'json',
  })
  expect(MOTION_ART.atlas).toEqual({
    path: 'world/avatars/derived-v1/motion-candidates-v1/avatar-candidates-64-1cd8f05af8d76d58.png',
    sha256: 'sha256:1cd8f05af8d76d58243e2f6e143961953b4d4291fdd553df8c8d33aabd700dbd', bytes: 470010, kind: 'png',
  })
  // The reviewed manifest still pins the same atlas and descriptor.
  expect(REVIEWED_MOTION.manifestId).toBe(pin.manifest_id)
  expect(REVIEWED_MOTION.manifestVersion).toBe(2)
})

it('validates the real pinned artwork through the profile entity', async () => {
  const verified = await loadArt()
  expect(verified.descriptor.atlas.file).toBe('avatar-candidates-64-1cd8f05af8d76d58.png')
  expect(verified.descriptor.atlas.width).toBe(1280)
  expect(verified.descriptor.atlas.height).toBe(1856)
  expect(verified.descriptor.sourcePack).toEqual({ id: 'mcr_user_characters_20261103', revision: 3 })
})

// ---------------------------------------------------------------------------
// T-704: no event gate in the editor
// ---------------------------------------------------------------------------

it('the editor has no event gate: it renders for any account-scoped owner', async () => {
  // The concert editor needed an event-scoped owner. The profile owner works
  // with an account alone, and the runtime does not read the event id.
  const f = await setup()
  cleanups.push(f.dispose)
  const editor = await owner(f)
  editor.start(); await settle()
  expect(editor.controller.read().phase).toBe('ready')
  expect(editor.controller.read().catalog).not.toBeNull()
  // The runtime's own scope is the profile scope: it has no event member, and
  // the runtime never reads one.
  const runtimeScope = editor.controller.runtime.scope as Record<string, unknown>
  for (const key of ['userId', 'generation', 'policyGeneration', 'gate']) expect(runtimeScope).toHaveProperty(key)
  // The runtime never reads an event: it is current even though the canonical
  // session scope still carries one.
  expect(editor.controller.runtime.isCurrent()).toBe(true)
  const runtimeSource = readFileSync(new FileURL('../../../../src/entities/emo-avatar/runtime.ts', import.meta.url), 'utf8')
  expect(codeOnly(runtimeSource)).not.toContain('eventId')
})

it('the editor source carries no concert wording and no event gate', () => {
  const source = readFileSync(new FileURL('../ui/ProfileAvatarEditor.tsx', import.meta.url), 'utf8')
  const code = codeOnly(source)
  for (const forbidden of ['ConcertWindow', 'ConcertAction', 'concert', 'Concert', 'eventId', 'event_id', '/events/', 'World']) {
    expect(code, `editor code must not mention ${forbidden}`).not.toContain(forbidden)
  }
  // The editor never uses an event to decide whether to render.
  expect(code).not.toContain('eventGate')
  expect(code).not.toContain('motionAvatar')
})
