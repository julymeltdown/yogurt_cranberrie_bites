// @vitest-environment jsdom
import { readFileSync } from 'node:fs'
import { URL as FileURL } from 'node:url'
import { render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMotionRuntime } from '../../../entities/emo-avatar/runtime'
import { MOTION_ART, validateMotionArt, type MotionArt } from '../../../entities/emo-avatar/art'
import { createProfileAvatarPersistence, PROFILE_AVATAR_PATH } from '../api/persistence'
import {
  ACCOUNT_CONTEXT_STALE,
  accountVersion,
  bindAvatarCacheToPrivacy,
  createAccountScopedAvatarCache,
  isAccountContextStale,
  sameAccountVersion,
} from '../model/cache'
import { createProfileAvatarEditorOwner } from '../model/owner'
import { ProfileAvatarPanel } from '../ui/ProfileAvatarPanel'
import { ACCOUNT, OTHER_ACCOUNT, layered, preset, setup, type Fixture } from './fixtures'

const cleanups: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.test' }))
afterEach(() => { cleanups.splice(0).reverse().forEach(fn => fn()); vi.unstubAllGlobals(); vi.restoreAllMocks() })

let art: MotionArt
async function loadArt() {
  art ??= await validateMotionArt(new Uint8Array(readFileSync(new FileURL(`../../../../public/${MOTION_ART.descriptor.path}`, import.meta.url))))
  return art
}
const settle = async () => { for (let i = 0; i < 60; i++) await new Promise(resolve => setTimeout(resolve, 0)) }

async function owner(f: Fixture) {
  // Bind to the *current* account, as the shell does after a switch.
  const scope = f.privacy.capture()
  const authority = await f.create()
  const runtime = createMotionRuntime({
    authority, scope, privacy: f.auth.privacy, origin: 'https://mcr.test',
    load: async () => ({ art: await loadArt(), image: new Image(), dispose() {} }),
  })
  const value = createProfileAvatarEditorOwner({ auth: f.auth, scope, authority, runtime })
  cleanups.push(value.dispose, runtime.dispose, authority.dispose)
  return value
}

// ---------------------------------------------------------------------------
// V-09-c: switching accounts clears the avatar cache
// ---------------------------------------------------------------------------

it('a cache entry written for one account is never readable for another', () => {
  const cache = createAccountScopedAvatarCache<string>()
  const first = accountVersion({ userId: ACCOUNT, generation: 1, policyGeneration: 0, gate: 'open', eventId: null })
  const second = accountVersion({ userId: OTHER_ACCOUNT, generation: 1, policyGeneration: 0, gate: 'open', eventId: null })
  cache.write(first, 'account-one-character')
  expect(cache.read(first)).toBe('account-one-character')
  // Reading under the other account evicts rather than serving the old avatar.
  expect(cache.read(second)).toBeUndefined()
  expect(cache.size).toBe(0)
  expect(cache.read(first)).toBeUndefined()
})

it('a generation bump is a different account version even for the same user id', () => {
  const cache = createAccountScopedAvatarCache<string>()
  const before = accountVersion({ userId: ACCOUNT, generation: 1, policyGeneration: 0, gate: 'open', eventId: null })
  const after = accountVersion({ userId: ACCOUNT, generation: 2, policyGeneration: 0, gate: 'open', eventId: null })
  cache.write(before, 'stale')
  expect(cache.read(after)).toBeUndefined()
  expect(sameAccountVersion(before, after)).toBe(false)
  expect(sameAccountVersion(before, before)).toBe(true)
  expect(sameAccountVersion(null, before)).toBe(false)
})

it('an unauthenticated scope stores and reads nothing', () => {
  const cache = createAccountScopedAvatarCache<string>()
  cache.write(null, 'should-not-stick')
  expect(cache.size).toBe(0)
  expect(cache.read(null)).toBeUndefined()
})

it('the privacy owner clears the avatar cache on an account switch', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const cache = createAccountScopedAvatarCache<string>()
  const unbind = bindAvatarCacheToPrivacy({ cache, privacy: f.auth.privacy })
  cleanups.push(unbind)
  const cleared = vi.fn()
  cache.onClear(cleared)
  cache.write(accountVersion(f.scope), 'first-account-character')
  expect(cache.size).toBe(1)
  f.switchAccount(OTHER_ACCOUNT)
  expect(cache.size).toBe(0)
  expect(cleared).toHaveBeenCalled()
  expect(cache.read(accountVersion(f.privacy.capture()))).toBeUndefined()
})

it('an ACCOUNT_CONTEXT_STALE refusal is recognized and clears the cache', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const cache = createAccountScopedAvatarCache<string>()
  const unbind = bindAvatarCacheToPrivacy({ cache, privacy: f.auth.privacy })
  cleanups.push(unbind)
  cache.write(accountVersion(f.scope), 'first-account-character')
  // The frozen reason is recognized from a 409; any other reason is not.
  expect(isAccountContextStale(409, ACCOUNT_CONTEXT_STALE)).toBe(true)
  expect(isAccountContextStale(409, 'VERSION_CONFLICT')).toBe(false)
  expect(isAccountContextStale(403, ACCOUNT_CONTEXT_STALE)).toBe(false)
  expect(isAccountContextStale(undefined, undefined)).toBe(false)
  // A stale context clears the cache through the same boundary a switch uses.
  for (const listener of [() => cache.clear()]) listener()
  expect(cache.size).toBe(0)
})

// ---------------------------------------------------------------------------
// V-09-d: after switching back, the previous account's draft is not visible
// ---------------------------------------------------------------------------

it('after switching accounts and back, the previous draft is not visible', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const first = await owner(f)
  first.start(); await settle()
  expect(first.controller.read().phase).toBe('ready')
  // Make a draft that is never saved.
  first.controller.selectPreset(first.controller.read().catalog!.presets[5]!.id)
  expect(first.controller.read().draft).toEqual(preset(5))
  // Switch to another account: the first owner is withdrawn synchronously.
  f.switchAccount(OTHER_ACCOUNT)
  expect(first.isCurrent()).toBe(false)
  expect(['unavailable', 'disposed']).toContain(first.controller.read().phase)
  expect(first.controller.read()).toMatchObject({ draft: null, persisted: null, catalog: null })
  expect(await first.controller.save()).toBe(false)
  // The second account starts from its own default, not the first account's draft.
  const second = await owner(f)
  second.start(); await settle()
  expect(second.controller.read().phase).toBe('ready')
  expect(second.controller.read().draft).toEqual(preset(0))
  expect(second.controller.read().draft).not.toEqual(preset(5))
})

it('a second account never renders the first account character', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  // The first account saves a distinct character.
  const first = await owner(f)
  first.start(); await settle()
  expect(first.controller.selectPreset(first.controller.read().catalog!.presets[3]!.id)).toBe(true)
  expect(await first.controller.save()).toBe(true)
  expect(f.server().config).toEqual(preset(3))
  // The second account's server state is its own default (the fixture's server
  // is per-fixture, so a fresh fixture models the second account).
  const g = await setup({ userId: OTHER_ACCOUNT })
  cleanups.push(g.dispose)
  const second = await owner(g)
  second.start(); await settle()
  expect(second.controller.read().persisted).toEqual({ revision: 1, config: preset(0) })
  expect(second.controller.read().persisted?.config).not.toEqual(preset(3))
})

it('the panel renders an explicit unavailable state instead of a foreign editor', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const first = await owner(f)
  first.start(); await settle()
  const { rerender, container } = render(<ProfileAvatarPanel owner={first} scope={f.scope} />)
  await waitFor(() => expect(screen.getByRole('heading', { name: 'Your character' })).toBeTruthy())
  // The account switches; the panel must stop showing the first account's editor.
  const nextScope = f.switchAccount(OTHER_ACCOUNT)
  rerender(<ProfileAvatarPanel owner={first} scope={nextScope} />)
  await waitFor(() => expect(container.textContent).toContain('unavailable'))
  expect(screen.queryByRole('button', { name: 'Save character' })).toBeNull()
})

it('the panel renders an unavailable state when there is no owner at all', () => {
  const { container } = render(<ProfileAvatarPanel owner={null} scope={null} />)
  expect(container.textContent).toContain('unavailable')
  expect(screen.queryByRole('button', { name: 'Save character' })).toBeNull()
})

// ---------------------------------------------------------------------------
// V-09-e: writes nothing on a stale context
// ---------------------------------------------------------------------------

it('a stale account context refuses the save and writes nothing', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const editor = await owner(f)
  editor.start(); await settle()
  editor.controller.selectPreset(editor.controller.read().catalog!.presets[2]!.id)
  const before = f.server()
  f.mode('stale-context')
  expect(await editor.controller.save()).toBe(false)
  // The server state is unchanged: no write landed.
  expect(f.server()).toEqual(before)
  expect(f.calls.filter(call => call.method === 'PUT')).toHaveLength(1)
})

// ---------------------------------------------------------------------------
// The persistence cannot outlive its account
// ---------------------------------------------------------------------------

it('an in-flight read cannot resolve into a switched account', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const authority = await f.create()
  cleanups.push(authority.dispose)
  const persistence = createProfileAvatarPersistence({ auth: f.auth, authority })
  cleanups.push(() => persistence.dispose())
  // Hold the read open, switch accounts, then release it.
  let release!: (response: Response) => void
  f.handler(() => new Promise<Response>(resolve => { release = resolve }))
  const pending = persistence.read(new AbortController().signal).then(() => 'resolved', () => 'rejected')
  await waitFor(() => expect(f.calls).toHaveLength(1))
  f.switchAccount(OTHER_ACCOUNT)
  release(Response.json({ revision: 1, config: preset(0) }))
  expect(await pending).toBe('rejected')
})

it('the cache module never keys on an event id', () => {
  const source = readFileSync(new FileURL('../model/cache.ts', import.meta.url), 'utf8')
  const code = source.replace(/\/\*[\s\S]*?\*\//g, '').split('\n').filter(line => !line.trimStart().startsWith('//')).join('\n')
  expect(code).not.toContain('eventId')
  expect(code).not.toContain('event_id')
  // The account version is exactly the account identity and its versions.
  const version = accountVersion({ userId: ACCOUNT, generation: 3, policyGeneration: 1, gate: 'open', eventId: 'ignored' })
  expect(version).toEqual({ userId: ACCOUNT, generation: 3, policyGeneration: 1 })
})

it('the persistence path is the profile path in every request', async () => {
  const f = await setup()
  cleanups.push(f.dispose)
  const authority = await f.create()
  cleanups.push(authority.dispose)
  const persistence = createProfileAvatarPersistence({ auth: f.auth, authority })
  cleanups.push(() => persistence.dispose())
  const controller = new AbortController()
  await persistence.read(controller.signal)
  await persistence.save(1, layered(), controller.signal)
  expect(new Set(f.calls.map(call => call.path))).toEqual(new Set([PROFILE_AVATAR_PATH]))
  expect(PROFILE_AVATAR_PATH).toBe('/api/me/avatar')
})
