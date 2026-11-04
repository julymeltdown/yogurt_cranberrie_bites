import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { fixture, ids, settle } from './fixture'
import { delta, pixel, snapshot } from '../../../features/mural/sync/__tests__/fixture'
const owned: ReturnType<typeof fixture>[] = []
const setup = async () => { const f = fixture(); owned.push(f); const r = f.qualify(); f.lifecycle.bind(r.input); await settle(); r.scene.frame(); return { f, r, sync: f.owner.read().sync! } }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }))
afterEach(() => { owned.splice(0).reverse().forEach(f => f.dispose()); vi.unstubAllGlobals() })

it('retains a lost placement across world replacement and checks exactly the original key/result without another POST', async () => {
  const { f, r, sync } = await setup(); f.server.mode('lost'); sync.editor.select(2, 3); sync.editor.chooseColor(2); await sync.editor.confirm()
  const command = sync.editor.read().command!, old = f.lifecycle.read()!
  expect(sync.editor.read().phase).toBe('unknown')
  const binding = { ...f.binding, roomEpoch: ids.next, streamId: ids.other }
  f.setView({ ...f.view(), baseline: { ...f.view().baseline!, ...binding } })
  expect(old.signal.aborted).toBe(true); expect(sync.editor.read().command).toBe(command)
  expect(sync.editor.read().snapshot).toBeNull(); expect(r.scene.read().textures).toBe(0)
  const next = f.qualify(); next.authority.set({ binding, writable: true })
  f.lifecycle.bind({ ...next.input, binding }); await settle(); next.scene.frame()
  expect(f.owner.read().sync).toBe(sync); expect(sync.editor.read().command).toBe(command); expect(sync.read().revision).toBe(11)
  expect(sync.editor.select(4, 4)).toBe(false); await sync.editor.confirm()
  f.server.mode('ack'); f.server.resultAvailable(true); await sync.editor.check()
  expect(sync.editor.read().phase).toBe('confirmed'); expect(sync.editor.read().command).toBeNull()
  const posts = f.requests.filter(r => r.method === 'POST')
  expect(posts).toHaveLength(1); expect(posts[0]!.headers.get('Idempotency-Key')).toBe(command.key)
  expect(await posts[0]!.json()).toEqual(command.body)
  expect(f.requests.filter(r => r.url.endsWith(`/mural/commands/${command.key}/result`))).toHaveLength(1)
})

it('aborts a held snapshot before publication and ignores its late response after a new release is ready', async () => {
  const f = fixture(); owned.push(f); const r = f.qualify(), hold = f.hold(r => r.url.endsWith('/snapshot'))
  f.lifecycle.bind(r.input); const response = await hold.arrived.promise, sync = f.owner.read().sync!
  r.close(); expect(f.requests[0]!.signal.aborted).toBe(true); expect(sync.editor.read().snapshot).toBeNull()
  f.stopHold(); f.server.restore(snapshot({ revision: 20, snapshot_id: ids.next }))
  f.lifecycle.bind(f.qualify().input); await settle(); expect(sync.read().revision).toBe(20)
  hold.response.resolve(response); await settle(); expect(sync.read().revision).toBe(20)
  expect(r.scene.read().textures).toBe(0)
})

it('scene withdrawal during a committed held POST leaves an unknown original, and explicit recovery resolves it', async () => {
  const { f, r, sync } = await setup(), hold = f.hold(r => r.method === 'POST')
  sync.editor.select(2, 2); const pending = sync.editor.confirm(), response = await hold.arrived.promise
  const command = sync.editor.read().command!
  r.close(); expect(f.requests.find(r => r.method === 'POST')!.signal.aborted).toBe(true)
  hold.response.resolve(response); await pending; await settle()
  expect(sync.editor.read().phase).toBe('unknown'); expect(sync.editor.read().command).toBe(command)
  f.stopHold(); f.lifecycle.bind(f.qualify().input); await settle()
  f.server.resultAvailable(true); await sync.editor.check()
  expect(sync.editor.read().phase).toBe('confirmed'); expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(1)
})

it('two release-driven consumers converge, roll back a rejected preview and discard obsolete deltas after restore', async () => {
  const a = await setup(), b = await setup(), p11 = pixel(11), p12 = pixel(12)
  a.sync.receiveDelta(a.sync.lease()!, delta(10, [p11, p12]))
  b.sync.receiveDelta(b.sync.lease()!, delta(11, [p12])); expect(b.sync.read().phase).toBe('gap')
  b.sync.receiveDelta(b.sync.lease()!, delta(10, [p11])); b.sync.receiveDelta(b.sync.lease()!, delta(10, [p11, p12]))
  expect(a.sync.editor.read().snapshot!.pixels).toEqual(b.sync.editor.read().snapshot!.pixels)
  a.f.server.mode('rejected'); a.sync.editor.select(1, 1); await a.sync.editor.confirm()
  expect(a.sync.editor.read().selection).toBeNull(); expect(a.sync.read().revision).toBe(12)
  const old = b.sync.lease()!, restored = snapshot({ revision: 50, snapshot_id: ids.next, state: 'PAUSED' })
  for (const c of [a, b]) {
    c.f.server.restore(restored); c.sync.receiveDelta(c.sync.lease()!, delta(12, [], true, 50)); await settle(); c.r.scene.frame()
    expect(c.sync.read().revision).toBe(50); expect(c.sync.editor.read().selection).toBeNull(); expect(c.sync.editor.canEdit()).toBe(false)
  }
  expect(b.sync.receiveDelta(old, delta(50, [pixel(51)]))).toBe(false)
  expect(a.sync.editor.read().snapshot!.pixels).toEqual(b.sync.editor.read().snapshot!.pixels)
  expect(a.r.scene.read().rgba).toEqual(b.r.scene.read().rgba)
})

it('privacy invalidation erases the retained unknown command and private cooldown, rather than recovering it in another scope', async () => {
  const { f, r, sync } = await setup(); f.server.mode('lost'); sync.editor.select(1, 1); await sync.editor.confirm()
  expect(sync.editor.read().command).not.toBeNull()
  const count = f.requests.length; f.auth.privacy.changePolicy(f.scope, 'block')
  expect(sync.editor.read().command).toBeNull(); expect(sync.editor.read().snapshot).toBeNull()
  expect(f.lifecycle.isDisposed()).toBe(true); expect(r.scene.read().textures).toBe(0)
  await sync.editor.check(); await sync.editor.confirm(); expect(f.requests).toHaveLength(count)
})
