import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { entryFixture, drain, ids } from './fixtures'
const owned: ReturnType<typeof entryFixture>[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }))
afterEach(() => { owned.splice(0).forEach(f => f.dispose()); vi.unstubAllGlobals() })
const fixture = () => { const f = entryFixture(); owned.push(f); return f }
const settle = async () => { for (let i = 0; i < 20; i++) await drain() }

describe('FE33 shell mural entry', () => {
  it('fails closed without/mismatched authority and recovers through one existing sync/scene lease', async () => {
    const f = fixture(), owner = f.create(), r = f.release()
    expect(owner.read().sync).toBeNull(); expect(f.requests).toHaveLength(0)
    owner.bind({ ...r.value, scope: { ...f.scope, policyGeneration: 99 } }); await settle()
    expect(f.requests).toHaveLength(0); expect(r.authority.count()).toBe(0)
    owner.bind({ ...r.value, binding: { ...r.value.binding, roomEpoch: ids.next } }); await settle()
    expect(f.requests).toHaveLength(0); expect(owner.read().sync).toBeNull()
    owner.bind(r.value); await settle(); const sync = owner.read().sync!
    expect(sync.read().phase).toBe('ready'); r.scene.frame()
    expect(r.scene.read()).toMatchObject({ textures: 1, objects: 1, listeners: 1 }); expect(f.clock.count()).toBe(1)
    owner.bind(r.value); await settle(); expect(owner.read().sync).toBe(sync); expect(f.requests).toHaveLength(1)
    r.authority.set(null); expect(sync.editor.read().snapshot).toBeNull(); expect(owner.read().sync).toBeNull()
    expect(r.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 }); expect(f.clock.count()).toBe(0)
    r.authority.set({ binding: r.value.binding, writable: false }); await settle()
    expect(owner.read().sync).toBe(sync); expect(sync.editor.canEdit()).toBe(false)
    expect(sync.editor.select(2, 3)).toBe(false); await sync.editor.confirm()
    expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(0)
    r.authority.set({ binding: r.value.binding, writable: true }); expect(sync.editor.canEdit()).toBe(true)
    sync.editor.select(2, 3); sync.editor.chooseColor(2); await sync.editor.confirm()
    const post = f.requests.find(r => r.method === 'POST')!
    expect(new URL(post.url).pathname).toBe(`/api/concert/events/${ids.event}/mural/pixels`)
    expect(post.headers.get('Authorization')).toBe(`Bearer ${ids.user}`)
    expect(post.headers.get('Idempotency-Key')).toMatch(/^[a-f0-9-]{36}$/)
    expect(post.cache).toBe('no-store')
    expect(post.headers.get('X-MCR-Intent')).toBe('browser')
    expect(post.headers.get('Content-Type')).toBe('application/json')
    expect(await post.json()).toEqual({ expected_revision: 10, x: 2, y: 3, color_index: 2 })
    expect(sync.editor.read().phase).toBe('confirmed')
  })

  it('retains an unknown command across signal withdrawal and replacement; recovery reads the original without POST replay', async () => {
    const f = fixture(), r = f.release(), owner = f.create(r.value); await settle()
    const sync = owner.read().sync!; f.server.mode('lost'); sync.editor.select(2, 3); await sync.editor.confirm()
    const command = sync.editor.read().command!
    expect(sync.editor.read().phase).toBe('unknown')
    r.lifetime.abort(); expect(owner.read().sync).toBeNull(); expect(sync.editor.read().snapshot).toBeNull()
    expect(sync.editor.read().command).toBe(command); expect(r.authority.count()).toBe(0); expect(r.count()).toBe(0)
    const next = f.release(); owner.bind(next.value); await settle()
    expect(owner.read().sync).toBe(sync); expect(sync.editor.read().command).toBe(command)
    expect(sync.editor.select(9, 9)).toBe(false); await sync.editor.confirm()
    expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(1)
    f.server.mode('ack'); f.server.resultAvailable(true); await sync.editor.check()
    expect(sync.editor.read().phase).toBe('confirmed'); expect(sync.editor.read().command).toBeNull()
    expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(1)
    const gets = f.requests.filter(r => r.method === 'GET').map(r => new URL(r.url))
    expect(gets.some(u => u.pathname.endsWith('/commands') && u.searchParams.get('idempotency_key') === command.key)).toBe(true)
    expect(gets.some(u => u.pathname.endsWith(`/mural/commands/${command.key}/result`) && u.search === '')).toBe(true)
    expect(f.clock.count()).toBe(1); next.scene.frame(); expect(next.scene.read().textures).toBe(1)
    expect(r.scene.read().textures).toBe(0)
  })

  it('aborts and fences late snapshots and commands on release withdrawal, including ignored fetch aborts', async () => {
    const f = fixture(), r = f.release(), hold = f.hold(r => r.url.endsWith('/snapshot')), owner = f.create(r.value)
    const response = await hold.arrived.promise, sync = owner.read().sync!
    r.lifetime.abort(); expect(f.requests[0]!.signal.aborted).toBe(true)
    hold.response.resolve(response); await settle()
    expect(sync.editor.read().snapshot).toBeNull(); expect(f.clock.count()).toBe(0); expect(r.scene.read().textures).toBe(0)
    f.stopHold(); const next = f.release(); owner.bind(next.value); await settle()
    const post = f.hold(r => r.method === 'POST'); sync.editor.select(2, 3); const pending = sync.editor.confirm()
    const ack = await post.arrived.promise
    next.lifetime.abort(); expect(f.requests.find(r => r.method === 'POST')!.signal.aborted).toBe(true)
    post.response.resolve(ack); await pending; await settle()
    expect(sync.editor.read().phase).toBe('unknown'); expect(sync.editor.read().snapshot).toBeNull()
    expect(sync.editor.read().command).not.toBeNull(); expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(1)
  })

  it.each(['block', 'identity', 'event', 'suspended'] as const)('scrubs raster/command and captured callbacks synchronously on %s', async change => {
    const f = fixture(), r = f.release(true), owner = f.create(r.value); await settle(); const sync = owner.read().sync!
    r.scene.frame(); f.server.mode('lost'); sync.editor.select(1, 1); await sync.editor.confirm()
    expect(sync.editor.read().command).not.toBeNull()
    if (change === 'identity' || change === 'event') f.auth.session.replace(f.auth.session.read().generation,
      { userId: change === 'identity' ? ids.other : ids.user, eventId: change === 'event' ? ids.next : ids.event, accessToken: 'synthetic-replacement' })
    else f.auth.privacy.changePolicy(f.scope, change)
    expect(owner.isDisposed()).toBe(true); expect(owner.read().sync).toBeNull()
    expect(sync.editor.read().snapshot).toBeNull(); expect(sync.editor.read().command).toBeNull()
    expect(r.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 }); expect(r.count()).toBe(0)
    expect(f.clock.count()).toBe(0); expect(r.authority.count()).toBe(0)
    const count = f.requests.length; await sync.editor.check(); await sync.editor.confirm(); owner.bind(f.release().value)
    expect(f.requests).toHaveLength(count)
  })

  it('withdraws on scene close and captures original methods instead of adopting mutated release objects', async () => {
    const f = fixture(), r = f.release(true), owner = f.create(r.value); await settle(); const sync = owner.read().sync!
    r.scene.frame(); r.value.authority.read = () => { throw new Error('mutated caller method') }
    expect(await sync.refresh()).toBe(true) // captured original read remains the admitted source
    r.close(); expect(owner.read().sync).toBeNull(); expect(sync.editor.read().snapshot).toBeNull()
    expect(r.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 }); expect(r.authority.count()).toBe(0)
  })

  it('requires a newly qualified matching scene after an epoch change and preserves the unresolved command', async () => {
    const f = fixture(), r = f.release(), owner = f.create(r.value); await settle(); const sync = owner.read().sync!
    f.server.mode('lost'); sync.editor.select(1, 1); await sync.editor.confirm(); const command = sync.editor.read().command
    const binding = { ...r.value.binding, roomEpoch: ids.next, streamId: ids.other }
    r.authority.set({ binding, writable: true })
    const reads = f.requests.length
    expect(owner.read().sync).toBeNull(); expect(sync.editor.read().snapshot).toBeNull(); expect(f.clock.count()).toBe(0)
    await sync.refresh(); await sync.editor.confirm(); expect(f.requests).toHaveLength(reads)
    const next = f.release(); next.authority.set({ binding, writable: true })
    owner.bind({ ...next.value, binding }); await settle(); next.scene.frame()
    expect(owner.read().sync).toBe(sync); expect(sync.editor.read().command).toBe(command)
    expect(next.scene.read()).toMatchObject({ textures: 1, objects: 1, listeners: 1 }); expect(f.clock.count()).toBe(1)
    expect(r.authority.count()).toBe(0); expect(r.count()).toBe(0); expect(r.scene.read().textures).toBe(0)
    expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(1)
  })

  it('releases partial subscriptions if an external subscribe/scene setup fails', async () => {
    const f = fixture(), r = f.release(), owner = f.create()
    owner.bind({ ...r.value, authority: { read: r.authority.read, subscribe() { throw new Error('synthetic source setup fault') } } })
    expect(owner.read().sync).toBeNull(); expect(r.count()).toBe(0); expect(f.clock.count()).toBe(0)
    expect(f.requests).toHaveLength(0)
    owner.bind(r.value); await settle(); expect(owner.read().sync?.read().phase).toBe('ready')
  })

  it('uses epoch receipt expiry independently from the monotonic sync clock', async () => {
    const f = fixture(), r = f.release(), owner = f.create(r.value, () => Date.parse('2026-11-06T00:00:00Z')); await settle()
    const sync = owner.read().sync!; f.server.mode('lost'); sync.editor.select(1, 1); await sync.editor.confirm()
    const command = sync.editor.read().command; expect(f.clock.now()).toBe(0)
    f.server.mode('ack'); f.server.resultAvailable(true); await sync.editor.check()
    expect(sync.editor.read().command).toBe(command); expect(sync.editor.read().phase).toBe('unknown')
    expect(f.requests.some(r => r.url.endsWith('/result'))).toBe(false)
    expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(1)
  })

  it('rechecks the captured authority at dispatch even before its change notification arrives', async () => {
    const f = fixture(), r = f.release(); let current = r.authority.read()
    const owner = f.create({ ...r.value, authority: { read: () => current, subscribe: r.authority.subscribe } }); await settle()
    const sync = owner.read().sync!; sync.editor.select(1, 1)
    current = { binding: r.value.binding, writable: false }
    await sync.editor.confirm()
    expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(0)
    expect(sync.editor.read().command).not.toBeNull()
  })
})
