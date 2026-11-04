// @vitest-environment jsdom
import { readFile } from 'node:fs/promises'
import { URL as FileURL } from 'node:url'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeAll, beforeEach, expect, it, vi } from 'vitest'
import { MOTION_ART } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { createMotionRuntime } from '../../../../entities/avatar/runtime/motion-adoption/runtime'
import { createReviewedMotionEditorOwner } from '../../motion-adoption/production-owner'
import { MotionEditor } from '../../motion-adoption/MotionEditor'
import { preset, setup } from '../reviewed-production/fixtures'

const cleanup: (() => void)[] = [], resources = new Map<string, Uint8Array<ArrayBuffer>>()
beforeAll(async () => {
  for (const resource of [MOTION_ART.descriptor, MOTION_ART.atlas]) resources.set(resource.path,
    new Uint8Array(await readFile(new FileURL(`../../../../../public/${resource.path}`, import.meta.url))))
})
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('Image', class { src = ''; async decode() {} })
  vi.stubGlobal('URL', class extends URL { static createObjectURL() { return 'blob:continuity-decoder' } static revokeObjectURL() {} })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ drawImage: vi.fn(), clearRect: vi.fn(), fillRect: vi.fn(),
    save: vi.fn(), restore: vi.fn(), beginPath: vi.fn(), rect: vi.fn(), clip: vi.fn() } as unknown as CanvasRenderingContext2D)
})
afterEach(async () => { await act(async () => cleanup.splice(0).reverse().forEach(fn => fn())); vi.restoreAllMocks(); vi.unstubAllGlobals() })
async function fixture(failInitialRead = false) {
  const f = await setup(); cleanup.push(f.dispose)
  const authority = await f.create()
  if (failInitialRead) f.handler(() => Response.json({ code: 'UNAVAILABLE', message: 'Unavailable', details: {} }, { status: 503 }))
  const runtime = createMotionRuntime({ authority, scope: f.scope, privacy: f.auth.privacy, origin: location.origin,
    fetch: async input => {
      const url = String(input), bytes = resources.get(new URL(url).pathname.slice(1))
      if (!bytes) throw Error('Unapproved resource')
      const response = new Response(bytes, { headers: { 'Content-Type': url.endsWith('.png') ? 'image/png' : 'application/json', 'Content-Length': String(bytes.length) } })
      Object.defineProperty(response, 'url', { value: url }); return response
    },
  }); cleanup.push(runtime.dispose)
  const onCommitted = vi.fn(), owner = createReviewedMotionEditorOwner({ auth: f.auth, scope: f.scope, authority, runtime, onCommitted })
  cleanup.push(owner.dispose)
  await act(async () => { owner.start(); await vi.waitFor(() => expect(owner.controller.read().phase).toBe('ready')) })
  const host = document.createElement('div'); document.body.append(host); const root = createRoot(host)
  await act(async () => root.render(<MotionEditor controller={owner.controller} />))
  cleanup.push(() => { root.unmount(); host.remove() })
  const button = (name: string) => {
    const found = [...host.querySelectorAll('button')].find(node => node.getAttribute('aria-label') === name || node.textContent === name)
    if (!found) throw Error(`Missing ${name}`)
    return found
  }
  const click = async (name: string) => { await act(async () => button(name).click()) }
  return { ...f, owner, onCommitted, host, button, click,
    writes: () => f.calls.filter(call => call.method === 'PUT'),
    edit: async () => { await act(async () => owner.controller.selectPreset(owner.controller.read().catalog!.presets[1]!.id)) },
  }
}

it('Cancel visibly discards a draft and leaves the persisted outfit untouched without a mutation or consumer publication', async () => {
  const f = await fixture(), saved = structuredClone(f.server())
  await f.edit(); expect(f.owner.controller.read().draft).not.toEqual(saved.config)
  expect(f.button('Cancel changes').textContent).toBe('Cancel')
  await f.click('Cancel changes')
  expect(f.owner.controller.read()).toMatchObject({ persisted: saved, draft: saved.config, needsReconciliation: false })
  expect(f.server()).toEqual(saved); expect(f.writes()).toHaveLength(0); expect(f.onCommitted).not.toHaveBeenCalled()
  expect(f.button('Save outfit').disabled).toBe(true)
})
it('reload admits a newer server outfit without losing the draft; Cancel after Keep uses that latest persisted outfit', async () => {
  const f = await fixture(); await f.edit(); const draft = f.owner.controller.read().draft
  const latest = { revision: 5, config: preset(3) }; f.setServer(latest)
  await act(async () => { expect(await f.owner.controller.reload()).toBe(true) })
  expect(f.owner.controller.read()).toMatchObject({ draft, candidate: latest, needsReconciliation: true })
  expect(f.button('Cancel changes').disabled).toBe(true); expect(f.button('Save outfit').disabled).toBe(true)
  await f.click('Keep my draft'); expect(f.owner.controller.read()).toMatchObject({ persisted: latest, draft })
  await f.click('Cancel changes'); expect(f.owner.controller.read().draft).toEqual(latest.config)
  expect(f.onCommitted).toHaveBeenCalledExactlyOnceWith(latest, f.scope); expect(f.writes()).toHaveLength(0)
})
it('a stale same-version changed receipt cannot replace the draft or unlock Save; a newer receipt can be explicitly adopted', async () => {
  const f = await fixture(); await f.edit(); const draft = f.owner.controller.read().draft
  f.setServer({ revision: 1, config: preset(4) })
  await act(async () => { expect(await f.owner.controller.reload()).toBe(false) })
  expect(f.owner.controller.read()).toMatchObject({ draft, candidate: null, needsReconciliation: true, notice: 'load-failed' })
  expect(f.button('Save outfit').disabled).toBe(true)
  const latest = { revision: 2, config: preset(4) }; f.setServer(latest)
  await f.click('Reload character'); await f.click('Use saved outfit')
  expect(f.owner.controller.read()).toMatchObject({ persisted: latest, draft: latest.config, needsReconciliation: false })
  expect(f.writes()).toHaveLength(0)
})
it('an initial participant read failure recovers through the actual Reload control without inventing a candidate or saving', async () => {
  const f = await fixture(true)
  expect(f.owner.controller.read()).toMatchObject({ draft: null, persisted: null, notice: 'load-failed' })
  f.handler(null); await f.click('Reload character')
  expect(f.owner.controller.read()).toMatchObject({ persisted: f.server(), draft: f.server().config, candidate: null, needsReconciliation: false })
  expect(f.writes()).toHaveLength(0)
})
it('a real optimistic conflict preserves the draft until explicit reconciliation and Cancel never retries the rejected write', async () => {
  const f = await fixture(); await f.edit(); const draft = f.owner.controller.read().draft
  const latest = { revision: 3, config: preset(5) }; f.setServer(latest)
  await f.click('Save outfit')
  expect(f.owner.controller.read()).toMatchObject({ draft, needsReconciliation: true, notice: 'conflict' })
  expect(f.writes()).toHaveLength(1); expect(f.button('Save outfit').disabled).toBe(true)
  await f.click('Check latest saved outfit'); await f.click('Keep my draft'); await f.click('Cancel changes')
  expect(f.owner.controller.read()).toMatchObject({ persisted: latest, draft: latest.config, needsReconciliation: false })
  expect(f.server()).toEqual(latest); expect(f.writes()).toHaveLength(1)
})
it('an uncertain committed response is read back and adopted without a duplicate save', async () => {
  const f = await fixture(); await f.edit(); f.mode('uncertain')
  await f.click('Save outfit'); const saved = structuredClone(f.server())
  expect(f.owner.controller.read()).toMatchObject({ notice: 'uncertain', needsReconciliation: true })
  f.mode('ok'); await f.click('Check latest saved outfit'); await f.click('Use saved outfit')
  expect(f.owner.controller.read()).toMatchObject({ persisted: saved, draft: saved.config, needsReconciliation: false })
  expect(f.writes()).toHaveLength(1); expect(f.button('Save outfit').disabled).toBe(true)
})
it('an item outside the admitted catalog cannot replace the draft, and Cancel returns to the admitted saved preset', async () => {
  const f = await fixture(); await f.edit(); const draft = f.owner.controller.read().draft
  await act(async () => { expect(await f.owner.controller.choose({ mode: 'wardrobe', body: 'foreign-body', hair: 'hair-0', eyes: 'eyes-0', outfit: null, accessory: null })).toBe(false) })
  expect(f.owner.controller.read()).toMatchObject({ draft, notice: 'invalid-selection' })
  await f.click('Cancel changes'); expect(f.owner.controller.read().draft).toEqual(f.server().config)
  expect(f.writes()).toHaveLength(0)
})
