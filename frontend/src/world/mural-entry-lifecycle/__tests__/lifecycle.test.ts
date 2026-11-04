import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fixture, ids, settle } from './fixture'
import type { MuralEntryQualification } from '../index'
const owned: ReturnType<typeof fixture>[] = []
const setup = (connectEntry = true) => { const f = fixture(connectEntry); owned.push(f); return f }
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }))
afterEach(() => { owned.splice(0).reverse().forEach(f => f.dispose()); vi.unstubAllGlobals() })

describe('explicit incumbent world qualification', () => {
  it('publishes a captured frozen release only after explicit anchor/binding qualification, without promoting read-only authority', async () => {
    const f = setup(), r = f.qualify()
    expect(f.lifecycle.read()).toBeNull(); expect(f.calls).toHaveLength(0); expect(f.requests).toHaveLength(0)
    r.authority.set({ binding: f.binding, writable: false })
    expect(f.lifecycle.bind(r.input)).toBe(true); await settle(); r.scene.frame()
    const release = f.lifecycle.read()!, sync = f.owner.read().sync!
    expect(Object.isFrozen(release)).toBe(true); expect(Object.isFrozen(release.binding)).toBe(true)
    expect(f.calls).toEqual([{ anchor: r.input.anchor, key: 'fixture-only-wall' }]); expect(Object.isFrozen(f.calls[0]!.anchor)).toBe(true)
    expect(sync.editor.canEdit()).toBe(false); expect(sync.editor.select(1, 1)).toBe(false)
    await sync.editor.confirm(); expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(0)
    expect(f.lifecycle.bind(r.input)).toBe(true); expect(f.lifecycle.read()).toBe(release); expect(f.calls).toHaveLength(1)
    expect(r.scene.read()).toMatchObject({ textures: 1, objects: 1, listeners: 1 })
  })

  it.each(['roomId', 'roomEpoch', 'streamId', 'eventId'] as const)('withdraws on world %s replacement and requires a fresh matching qualification', async field => {
    const f = setup(), r = f.qualify(); f.lifecycle.bind(r.input); await settle(); r.scene.frame()
    const release = f.lifecycle.read()!, sync = f.owner.read().sync!
    f.setView({ ...f.view(), baseline: { ...f.view().baseline!, [field]: ids.next } })
    expect(release.signal.aborted).toBe(true); expect(release.authority.read()).toBeNull()
    expect(f.lifecycle.read()).toBeNull(); expect(f.owner.read().sync).toBeNull(); expect(sync.editor.read().snapshot).toBeNull()
    expect(r.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 }); expect(r.count()).toBe(0); expect(r.authority.count()).toBe(0)
    expect(f.clock.count()).toBe(0); expect(f.lifecycle.bind(r.input)).toBe(false)
    f.setView({ ...f.view(), baseline: { ...f.view().baseline!, ...f.binding } }); expect(f.lifecycle.read()).toBeNull()
    const next = f.qualify(); expect(f.lifecycle.bind(next.input)).toBe(true); await settle()
    expect(f.owner.read().sync).toBe(sync); expect(sync.read().phase).toBe('ready')
  })

  it.each(['hidden', 'resync', 'closed', 'no-baseline', 'no-world', 'identity'] as const)('invalidates %s immediately without reconnecting or reusing the old release', async mode => {
    const f = setup(), r = f.qualify(); f.lifecycle.bind(r.input); await settle(); r.scene.frame(); const release = f.lifecycle.read()!
    const view = f.view()
    f.setView(mode === 'no-world' ? null : mode === 'hidden' ? { ...view, visible: false } : mode === 'no-baseline' ? { ...view, baseline: null }
      : mode === 'identity' ? { ...view, scope: { ...view.scope, userId: ids.other } } : { ...view, phase: mode })
    expect(release.signal.aborted).toBe(true); expect(f.lifecycle.read()).toBeNull(); expect(f.calls).toHaveLength(1)
    expect(r.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 })
    f.setView(view); expect(f.lifecycle.read()).toBeNull(); expect(f.lifecycle.bind(r.input)).toBe(false)
  })

  it.each(['policyGeneration', 'generation', 'eventId', 'userId', 'gate'] as const)('rejects mismatched qualification %s before touching the scene', field => {
    const f = setup(), r = f.qualify()
    const scope = { ...r.input.scope, [field]: field === 'gate' ? 'suspended' : field.includes('Generation') || field === 'generation' ? 99 : ids.other }
    expect(f.lifecycle.bind({ ...r.input, scope } as MuralEntryQualification)).toBe(false)
    expect(f.calls).toHaveLength(0); expect(f.requests).toHaveLength(0)
  })

  it.each([{ x: NaN }, { y: Infinity }, { width: 0 }, { height: -1 }, { width: 2049 }, { depth: NaN }])('rejects malformed anchor %j without default coordinates', patch => {
    const f = setup(), r = f.qualify()
    expect(f.lifecycle.bind({ ...r.input, anchor: { ...r.input.anchor, ...patch } })).toBe(false)
    expect(f.calls).toHaveLength(0)
  })

  it('fails closed on missing authority/bad binding/key/aborted supplier and never invents a write capability', () => {
    const f = setup(), r = f.qualify(); r.authority.set(null)
    expect(f.lifecycle.bind(r.input)).toBe(false)
    r.authority.set({ binding: f.binding, writable: false })
    expect(f.lifecycle.bind({ ...r.input, textureKey: '../wall' })).toBe(false)
    expect(f.lifecycle.bind({ ...r.input, binding: { ...f.binding, roomEpoch: 'invalid' } })).toBe(false)
    r.lifetime.abort(); expect(f.lifecycle.bind({ ...r.input })).toBe(false); expect(f.calls).toHaveLength(0)
  })

  it('rechecks unannounced world/authority loss at dispatch and fences previously captured scene methods', async () => {
    const f = setup(), r = f.qualify(); f.lifecycle.bind(r.input); await settle(); r.scene.frame()
    const release = f.lifecycle.read()!, sync = f.owner.read().sync!
    sync.editor.select(2, 2); f.setView({ ...f.view(), visible: false }, false)
    await sync.editor.confirm()
    expect(f.requests.filter(r => r.method === 'POST')).toHaveLength(0); expect(release.signal.aborted).toBe(true)
    expect(() => release.scene.texture()).toThrow('withdrawn'); expect(release.authority.read()).toBeNull()
    expect(r.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 })
  })

  it('captures supplier methods/anchor instead of adopting caller mutation; read-only changes do not replace the entry owner', async () => {
    const f = setup(), r = f.qualify(); f.lifecycle.bind(r.input); await settle(); const sync = f.owner.read().sync!
    r.input.authority.read = () => { throw Error('mutated') }
    ;(r.input.anchor as { x: number }).x = 999
    expect(f.lifecycle.read()?.authority.read()?.writable).toBe(true); expect(f.calls[0]!.anchor.x).toBe(16)
    r.authority.set({ binding: f.binding, writable: false }); expect(sync.editor.canEdit()).toBe(false)
    expect(f.owner.read().sync).toBe(sync); expect(f.calls).toHaveLength(1)
  })
})

describe('owned withdrawal and failure cleanup', () => {
  it.each(['abort', 'scene', 'authority', 'dispose', 'null'] as const)('%s clears resources even without an entry consumer or with throwing observers', mode => {
    const f = setup(false), r = f.qualify(); f.lifecycle.subscribe(() => { throw Error('observer') }); f.lifecycle.bind(r.input)
    const release = f.lifecycle.read()!, texture = release.scene.texture(), object = release.scene.object(texture)
    release.scene.onFrame(() => {}); release.scene.onClose(() => { throw Error('close observer') })
    if (mode === 'abort') r.lifetime.abort()
    else if (mode === 'scene') r.close()
    else if (mode === 'authority') r.authority.set(null)
    else if (mode === 'null') f.lifecycle.bind(null)
    else f.lifecycle.dispose()
    expect(release.signal.aborted).toBe(true); expect(f.lifecycle.read()).toBeNull()
    expect(r.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 }); expect(r.count()).toBe(0); expect(r.authority.count()).toBe(0)
    object.dispose(); texture.dispose(); expect(r.scene.read().textures).toBe(0)
  })

  it('cleans partial synchronous-close and throwing subscription setup and can accept a fresh qualification', async () => {
    const f = setup(), r = f.qualify()
    expect(f.lifecycle.bind({ ...r.input, scene: () => ({ ...r.value.scene, onClose(fn) { fn(); return () => {} } }) })).toBe(false)
    expect(f.lifecycle.read()).toBeNull(); expect(r.authority.count()).toBe(0)
    expect(f.lifecycle.bind({ ...r.input, authority: { read: r.authority.read, subscribe() { throw Error('setup') } } })).toBe(false)
    expect(r.count()).toBe(0); expect(f.lifecycle.read()).toBeNull()
    const next = f.qualify(); expect(f.lifecycle.bind(next.input)).toBe(true); await settle(); expect(f.owner.read().sync?.read().phase).toBe('ready')
  })

  it('withdraws on a scene allocation/upload failure and releases resources when allocation synchronously revokes authority', () => {
    const f = setup(false), r = f.qualify()
    f.lifecycle.bind({ ...r.input, scene: () => ({ ...r.value.scene, object() { throw Error('allocation failure') } }) })
    const release = f.lifecycle.read()!, texture = release.scene.texture()
    expect(() => release.scene.object(texture)).toThrow('allocation failure')
    expect(release.signal.aborted).toBe(true); expect(r.scene.read().textures).toBe(0)
    const next = f.qualify()
    f.lifecycle.bind({ ...next.input, scene: () => ({ ...next.value.scene, texture() {
      const value = next.value.scene.texture(); next.authority.set(null); return value
    } }) })
    expect(() => f.lifecycle.read()!.scene.texture()).toThrow('withdrawn')
    expect(next.scene.read().textures).toBe(0); expect(f.lifecycle.read()).toBeNull()
    const third = f.qualify()
    f.lifecycle.bind({ ...third.input, scene: () => ({ ...third.value.scene, texture() {
      const value = third.value.scene.texture(); return { dispose: value.dispose, replace() { throw Error('upload failure') } }
    } }) })
    const last = f.lifecycle.read()!, raster = last.scene.texture()
    expect(() => raster.replace(new Uint8ClampedArray(65536))).toThrow('upload failure')
    expect(last.signal.aborted).toBe(true); expect(third.scene.read().textures).toBe(0)
  })

  it('does not allocate from a reentrant withdrawal observer and ignores delayed callbacks from an old scene', async () => {
    const f = setup(), r = f.qualify(), next = f.qualify(); f.lifecycle.bind(r.input); await settle()
    const old = f.lifecycle.read()!, attempts: boolean[] = []
    old.signal.addEventListener('abort', () => attempts.push(f.lifecycle.bind(next.input)))
    r.lifetime.abort(); expect(attempts).toEqual([false]); expect(f.calls).toHaveLength(1)
    expect(f.lifecycle.bind(next.input)).toBe(true); await settle(); r.close()
    expect(f.lifecycle.read()?.signal.aborted).toBe(false); expect(f.calls).toHaveLength(2)
  })

  it.each(['block', 'suspended', 'identity', 'event', 'logout'] as const)('permanently disposes on %s and removes all borrowed subscriptions', async mode => {
    const f = setup(), r = f.qualify(); f.lifecycle.bind(r.input); await settle(); r.scene.frame()
    if (mode === 'block' || mode === 'suspended') f.auth.privacy.changePolicy(f.scope, mode)
    else f.auth.session.replace(f.scope.generation, mode === 'logout' ? null : { userId: mode === 'identity' ? ids.other : ids.user, eventId: mode === 'event' ? ids.next : ids.event, accessToken: 'fixture' })
    expect(f.lifecycle.isDisposed()).toBe(true); expect(f.lifecycle.read()).toBeNull(); expect(f.worldCount()).toBe(0)
    expect(r.count()).toBe(0); expect(r.authority.count()).toBe(0); expect(f.clock.count()).toBe(0)
    expect(r.scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 }); expect(f.lifecycle.bind(f.qualify().input)).toBe(false)
  })
})
