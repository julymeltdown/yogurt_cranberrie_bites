// @vitest-environment jsdom
import { readFile } from 'node:fs/promises'
import { URL as FileURL } from 'node:url'
import { act, StrictMode } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeAll, beforeEach, expect, it, vi } from 'vitest'
import { createMotionRuntime } from '../../../../entities/avatar/runtime/motion-adoption/runtime'
import { MOTION_ART } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { createReviewedMotionEditorOwner } from '../../motion-adoption/production-owner'
import { MotionEditor } from '../../motion-adoption/MotionEditor'
import { object } from '../../../auth/model/contract'
import { deferred, preset, setup } from './fixtures'

const cleanup: (() => void)[] = [], resources = new Map<string, Uint8Array<ArrayBuffer>>()
beforeAll(async () => {
  for (const resource of [MOTION_ART.descriptor, MOTION_ART.atlas]) resources.set(resource.path,
    new Uint8Array(await readFile(new FileURL(`../../../../../public/${resource.path}`, import.meta.url))))
})
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('Image', class { src = ''; async decode() {} })
  vi.stubGlobal('URL', class extends URL { static createObjectURL() { return 'blob:deterministic-decoder'; } static revokeObjectURL() {} })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ drawImage: vi.fn(), clearRect: vi.fn(), fillRect: vi.fn(), save: vi.fn(), restore: vi.fn(),
    beginPath: vi.fn(), rect: vi.fn(), clip: vi.fn() } as unknown as CanvasRenderingContext2D)
})
afterEach(async () => { await act(async () => { cleanup.splice(0).reverse().forEach(fn => fn()) }); vi.restoreAllMocks(); vi.unstubAllGlobals() })
async function fixture(start = true, canSave = true) {
  const f = await setup(); cleanup.push(f.dispose)
  if (!canSave) object(f.bundle().capabilities).mutations = { available: false, reason: 'FEATURE_DISABLED' }
  const authority = await f.create(), assetRequests: string[] = []
  const runtime = createMotionRuntime({ authority, scope: f.scope, privacy: f.auth.privacy, origin: location.origin,
    fetch: async input => {
      const url = String(input), path = new URL(url).pathname.slice(1), bytes = resources.get(path)
      if (!bytes) throw Error('Unapproved resource')
      assetRequests.push(path)
      const response = new Response(bytes, { headers: { 'Content-Type': path.endsWith('.png') ? 'image/png' : 'application/json', 'Content-Length': String(bytes.length) } })
      Object.defineProperty(response, 'url', { value: url }); return response
    },
  }); cleanup.push(runtime.dispose)
  const onCommitted = vi.fn(), options = { auth: f.auth, scope: f.scope, authority, runtime, onCommitted }
  const owner = createReviewedMotionEditorOwner(options); cleanup.push(owner.dispose)
  if (start) await act(async () => { owner.start(); await vi.waitFor(() => expect(owner.controller.read().phase).toBe('ready')) })
  return { ...f, authority, runtime, assetRequests, onCommitted, options, owner }
}
function mount(owner: Awaited<ReturnType<typeof fixture>>['owner'], strict = false) {
  const host = document.createElement('div'); document.body.append(host)
  let root: Root | null = createRoot(host)
  act(() => root!.render(strict ? <StrictMode><MotionEditor controller={owner.controller} /></StrictMode> : <MotionEditor controller={owner.controller} />))
  const unmount = () => { if (root) { act(() => root!.unmount()); root = null; host.remove() } }
  cleanup.push(unmount); return { host, unmount }
}
function button(host: HTMLElement, label: string) {
  const value = [...host.querySelectorAll('button')].find(item => item.getAttribute('aria-label') === label || item.textContent === label)
  if (!value) throw Error(`Missing button ${label}`)
  return value
}
async function click(host: HTMLElement, label: string) { await act(async () => { button(host, label).click() }) }
const reads = (f: Awaited<ReturnType<typeof fixture>>) => f.calls.filter(call => call.path.endsWith('/me') && call.method === 'GET')
const writes = (f: Awaited<ReturnType<typeof fixture>>) => f.calls.filter(call => call.method === 'PUT')

it('uses one canonical bootstrap, shared runtime and explicit initial participant read', async () => {
  const f = await fixture(false)
  expect(f.owner.controller.read().phase).toBe('idle'); expect(reads(f)).toHaveLength(0)
  expect(f.owner.controller.lifetime).toBe('owner'); expect(f.owner.controller.runtime).toBe(f.runtime)
  f.owner.start(); f.owner.start(); await vi.waitFor(() => expect(f.owner.controller.read().persisted).toEqual(f.server()))
  expect(f.calls.filter(call => call.path.endsWith('/bootstrap'))).toHaveLength(1)
  expect(reads(f)).toHaveLength(1); expect(f.assetRequests).toEqual([MOTION_ART.descriptor.path, MOTION_ART.atlas.path])
  expect(writes(f)).toHaveLength(0)
})
it('edits all actual layer choices through the production component and saves only on explicit Save', async () => {
  const f = await fixture(), view = mount(f.owner)
  const presets = f.owner.controller.read().catalog!.presets
  for (const choice of presets) { await click(view.host, `Choose ${choice.label}`); expect(f.owner.controller.read().draft).toEqual(choice.config) }
  await click(view.host, 'Dress up')
  for (const [layer, label, count] of [['hair', 'Hair', 6], ['eyes', 'Eyes', 4], ['outfit', 'Outfit', 6], ['accessory', 'Accessory', 4], ['body', 'Skin / body', 8]] as const) {
    await click(view.host, label)
    const select = view.host.querySelector<HTMLSelectElement>('.mcr-motion-select select')!
    const values = [...select.options].map(option => option.value).filter(Boolean)
    expect(values).toHaveLength(count)
    for (const value of values) {
      const before = f.authority.selection(f.owner.controller.read().draft!)
      await act(async () => { select.value = value; select.dispatchEvent(new Event('change', { bubbles: true })) })
      const selected = f.authority.selection(f.owner.controller.read().draft!)
      expect(selected).toEqual({ ...before, [layer]: value })
    }
    if (layer === 'outfit' || layer === 'accessory') {
      const before = f.authority.selection(f.owner.controller.read().draft!)
      await click(view.host, 'None')
      expect(f.authority.selection(f.owner.controller.read().draft!)).toEqual({ ...before, [layer]: null })
    }
  }
  expect(writes(f)).toHaveLength(0)
  await click(view.host, 'Save outfit'); expect(writes(f)).toHaveLength(1)
  expect(f.owner.controller.read()).toMatchObject({ persisted: f.server(), draft: f.server().config, notice: 'saved' })
  expect(f.onCommitted).toHaveBeenCalledExactlyOnceWith(f.server(), f.scope)
  await click(view.host, 'Random character'); expect(writes(f)).toHaveLength(1)
  await click(view.host, 'Reset draft'); expect(f.owner.controller.read().draft).toEqual(f.server().config)
})
it('view unmount/remount keeps the Account draft, one read and the shared art owner', async () => {
  const f = await fixture(), first = mount(f.owner)
  await click(first.host, 'Random character'); const draft = f.owner.controller.read().draft
  first.unmount(); expect(f.owner.controller.read()).toMatchObject({ phase: 'ready', draft })
  const second = mount(f.owner)
  expect(f.owner.controller.read().draft).toBe(draft); expect(button(second.host, 'Save outfit').disabled).toBe(false)
  expect(reads(f)).toHaveLength(1); expect(f.assetRequests).toHaveLength(2); expect(f.authority.isCurrent()).toBe(true)
})
it('view unmount cannot abort an explicit save or suppress its current-scope receipt', async () => {
  const f = await fixture(), view = mount(f.owner), waiting = deferred<Response>()
  await click(view.host, 'Random character'); const draft = f.owner.controller.read().draft!
  f.handler(request => request.method === 'PUT' ? waiting.promise : Promise.reject(Error('Unexpected read')))
  let save!: Promise<boolean>
  await act(async () => { save = f.owner.controller.save() })
  expect(f.owner.controller.read().phase).toBe('saving'); view.unmount()
  waiting.resolve(Response.json({ revision: 2, config: draft }))
  expect(await save).toBe(true); expect(f.onCommitted).toHaveBeenCalledExactlyOnceWith({ revision: 2, config: draft }, f.scope)
  expect(f.owner.controller.read()).toMatchObject({ phase: 'ready', draft, notice: 'saved' })
})
it.each(['conflict', 'uncertain'] as const)('keeps a production draft and requires explicit %s reconciliation', async mode => {
  const f = await fixture(); await f.owner.controller.random(); const draft = f.owner.controller.read().draft
  f.mode(mode); expect(await f.owner.controller.save()).toBe(false)
  expect(f.owner.controller.read()).toMatchObject({ draft, needsReconciliation: true, notice: mode })
  const attempts = writes(f).length; expect(await f.owner.controller.save()).toBe(false); expect(writes(f)).toHaveLength(attempts)
  f.mode('ok'); expect(await f.owner.controller.reload()).toBe(true)
  expect(f.owner.controller.resolveConflict('use-server')).toBe(true)
  expect(f.owner.controller.read()).toMatchObject({ draft: f.server().config, needsReconciliation: false })
})
it('Account disposal cancels late reads and clears its draft without closing another consumer lease or shared runtime', async () => {
  const f = await fixture(), other = await f.runtime.acquire(preset(0), { signal: new AbortController().signal })
  cleanup.push(other.dispose)
  const waiting = deferred<Response>(); f.handler(() => waiting.promise)
  const read = f.owner.controller.reload(); await vi.waitFor(() => expect(reads(f)).toHaveLength(2))
  f.owner.dispose(); f.owner.dispose(); waiting.resolve(Response.json(object(f.bundle().participant)))
  expect(await read).toBe(false); expect(f.owner.controller.read()).toMatchObject({ phase: 'disposed', draft: null, persisted: null })
  expect(f.onCommitted).not.toHaveBeenCalled(); expect(f.owner.isCurrent()).toBe(false)
  expect(f.runtime.isCurrent()).toBe(true); expect(f.authority.isCurrent()).toBe(true); expect(other.read()).not.toBeNull()
  f.owner.start(); expect(await f.owner.controller.save()).toBe(false)
  const next = createReviewedMotionEditorOwner(f.options); cleanup.push(next.dispose)
  f.handler(null); next.start(); await vi.waitFor(() => expect(next.controller.read().phase).toBe('ready'))
  expect(f.assetRequests).toHaveLength(2)
})
it('authority withdrawal clears the feature owner synchronously and rejects a late save receipt', async () => {
  const f = await fixture(); await f.owner.controller.random(); const draft = f.owner.controller.read().draft!
  const waiting = deferred<Response>(); f.handler(() => waiting.promise)
  const save = f.owner.controller.save(); await vi.waitFor(() => expect(writes(f)).toHaveLength(1))
  f.signal.abort(); waiting.resolve(Response.json({ revision: 2, config: draft }))
  expect(await save).toBe(false); expect(f.owner.controller.read()).toMatchObject({ phase: 'disposed', draft: null, catalog: null })
  expect(f.onCommitted).not.toHaveBeenCalled(); expect(f.owner.isCurrent()).toBe(false)
})
it.each(['userId', 'eventId', 'generation', 'policyGeneration', 'gate'] as const)('rejects mismatched owner %s without a read or mutation', async key => {
  const f = await fixture(false), value = f.scope[key]
  const scope = { ...f.scope, [key]: typeof value === 'number' ? value + 1 : value === 'open' ? 'closed' : `${value}-foreign` }
  expect(() => createReviewedMotionEditorOwner({ ...f.options, scope })).toThrow()
  expect(reads(f)).toHaveLength(0); expect(writes(f)).toHaveLength(0)
})
it('rejects copied authority, foreign Auth, mismatched runtime scope and disposed runtime', async () => {
  const f = await fixture(false)
  expect(() => createReviewedMotionEditorOwner({ ...f.options, authority: { ...f.authority } })).toThrow()
  expect(() => createReviewedMotionEditorOwner({ ...f.options, auth: { ...f.auth } })).toThrow()
  expect(() => createReviewedMotionEditorOwner({ ...f.options, runtime: { ...f.runtime, scope: { ...f.scope, policyGeneration: f.scope.policyGeneration + 1 } } })).toThrow()
  f.runtime.dispose(); expect(() => createReviewedMotionEditorOwner(f.options)).toThrow()
  expect(reads(f)).toHaveLength(0); expect(writes(f)).toHaveLength(0)
})

it('keeps permitted reads and drafts while mutation capability disables the production Save control', async () => {
  const f = await fixture(true, false), view = mount(f.owner)
  await click(view.host, 'Random character')
  expect(f.owner.controller.read().draft).not.toEqual(f.server().config)
  expect(button(view.host, 'Save outfit').disabled).toBe(true)
  await click(view.host, 'Save outfit'); expect(writes(f)).toHaveLength(0)
  expect(reads(f)).toHaveLength(1); expect(f.owner.isCurrent()).toBe(true)
})
it('reports callback refresh failure after an accepted save without duplicating the mutation', async () => {
  const f = await fixture(); f.onCommitted.mockImplementation(() => { throw Error('Projection refresh unavailable') })
  await f.owner.controller.random(); expect(await f.owner.controller.save()).toBe(true)
  expect(f.owner.controller.read()).toMatchObject({ persisted: f.server(), notice: 'saved-refresh-needed' })
  expect(writes(f)).toHaveLength(1)
})
it('Account disposal suppresses a late successful save while preserving the borrowed runtime', async () => {
  const f = await fixture(); await f.owner.controller.random(); const draft = f.owner.controller.read().draft!
  const waiting = deferred<Response>(); f.handler(() => waiting.promise)
  const save = f.owner.controller.save(); await vi.waitFor(() => expect(writes(f)).toHaveLength(1))
  f.owner.dispose(); waiting.resolve(Response.json({ revision: 2, config: draft }))
  expect(await save).toBe(false); expect(f.onCommitted).not.toHaveBeenCalled()
  expect(f.owner.controller.read()).toMatchObject({ phase: 'disposed', draft: null })
  expect(f.runtime.isCurrent()).toBe(true); expect(f.authority.isCurrent()).toBe(true)
})
it('canonical privacy policy revocation clears the Account draft and forbids restart', async () => {
  const f = await fixture(); await f.owner.controller.random()
  expect(f.auth.privacy.changePolicy(f.scope, 'consent-revoked')).toBe(true)
  expect(f.owner.controller.read()).toMatchObject({ phase: 'disposed', draft: null, persisted: null, catalog: null })
  f.owner.start(); expect(await f.owner.controller.random()).toBe(false); expect(await f.owner.controller.save()).toBe(false)
  expect(f.onCommitted).not.toHaveBeenCalled(); expect(writes(f)).toHaveLength(0)
})

it('StrictMode initial view replay does not restart or cancel the Account initialization', async () => {
  const f = await fixture(false), view = mount(f.owner, true)
  await act(async () => { await vi.waitFor(() => expect(f.owner.controller.read().phase).toBe('ready')) })
  expect(reads(f)).toHaveLength(1); expect(f.assetRequests).toHaveLength(2)
  expect(f.calls.filter(call => call.path.endsWith('/bootstrap'))).toHaveLength(1)
  view.unmount(); expect(f.owner.controller.read().phase).toBe('ready'); expect(f.runtime.isCurrent()).toBe(true)
})
