// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { waitFor } from '@testing-library/react'
import original from '../../../../../contracts/fixtures/core/bootstrap-open.json'
import { decodeBootstrap } from '../../../features/auth/model/bootstrap'
import { authResponse, capabilities } from '../../../features/auth/__tests__/fixtures/data'
import { createWorldEntryResolver, type WorldEntryResolverOptions } from '../resolver'
import { assetBytes, authHarness, bundle, eventId, inputConfiguration, requestFor, rooms, seal, userId, type Bundle } from './data'
import { prepareRelease } from '../../../world/composition/release'
import { immutable } from '../../../world/entities/replicated'
import { contentHash } from '../../../world/map/json'
const owners: ReturnType<typeof authHarness>[] = []
const setup = (handler?: Parameters<typeof authHarness>[0]) => { const h = authHarness(handler); owners.push(h); return h }
afterEach(() => { owners.splice(0).forEach(h => h.auth.dispose()); vi.useRealTimers() })
const options = (auth: WorldEntryResolverOptions['auth']): WorldEntryResolverOptions => ({ auth, origin: 'https://mcr.test', localInput: inputConfiguration })
const assetResponse = (url: string, bytes: Uint8Array) => {
  const response = new Response(bytes as BodyInit, { headers: { 'Content-Type': url.endsWith('.json') ? 'application/json' : 'image/png' } })
  Object.defineProperty(response, 'url', { value: url }); return response
}
describe('production world entry resolver boundaries', () => {
  it('uses the exact immutable bootstrap and persisted theme, one canonical protected no-store room read, and validated default', async () => {
    const h = setup(), request = await requestFor(h.auth), resolve = createWorldEntryResolver(options(h.auth))
    const loadBootstrap = vi.spyOn(h.auth, 'loadBootstrap'), result = await resolve(request)
    expect(result.bootstrap).toBe(request.bootstrap.document); expect(result.privacyScope).toEqual(request.privacyScope)
    expect(result.themeId).toBe('PARADE'); expect(result.roomId).toBe(rooms().default_room_id); expect(result.rooms).toEqual(rooms())
    expect(result.movementPolicy).toEqual(inputConfiguration.movementPolicy); expect(result.zoom).toBe(2)
    expect(Object.isFrozen(result.rooms.items[0])).toBe(true); expect(Object.isFrozen(result.movementPolicy)).toBe(true)
    expect(loadBootstrap).not.toHaveBeenCalled(); expect(h.requests).toHaveLength(1)
    const r = h.requests[0]!
    expect(new URL(r.url).pathname).toBe(`/api/concert/events/${eventId}/rooms`); expect(r.method).toBe('GET'); expect(r.cache).toBe('no-store')
    expect(r.headers.get('Authorization')).toBe('Bearer memory-only-test-token'); expect(r.headers.has('Idempotency-Key')).toBe(false)
    expect(h.auth.queryClient.getQueryCache().getAll()).toHaveLength(0)
    const prepared = await prepareRelease(result, request)
    expect(prepared.resources.roomId).toBe(result.roomId); expect(prepared.mapInput.resources).toBe(result.resources)
  })
  it('derives only the selected theme, map and selected-source atlas hash/path allowlist through the existing resource reader', async () => {
    const h = setup(), request = await requestFor(h.auth), calls: { url: string; init: RequestInit }[] = []
    const assetFetch: typeof fetch = async (input, init) => { const url = String(input); calls.push({ url, init: init! }); return assetResponse(url, url.endsWith('.json') ? assetBytes.json : assetBytes.png) }
    const result = await createWorldEntryResolver({ ...options(h.auth), assetFetch })(request)
    const doc = request.bootstrap.document as unknown as Bundle
    const selected = doc.theme_manifests.find(t => t.theme_id === 'PARADE')!
    for (const [r, kind] of [[doc.map_manifest.tilemap, 'json'], [selected.texture, 'png'], [doc.avatar_manifest.atlas.texture, 'png']] as const) {
      const resource = { path: r.path!, sha256: r.sha256!, kind }
      const data = await result.resources.read(resource, request.signal); expect(Array.from(data)).toEqual(Array.from(kind === 'json' ? assetBytes.json : assetBytes.png))
    }
    expect(calls).toHaveLength(3); expect(calls[0]!.init).toMatchObject({ credentials: 'omit', mode: 'same-origin', redirect: 'error', referrerPolicy: 'no-referrer' })
    const base = { path: selected.texture.path!, sha256: selected.texture.sha256!, kind: 'png' as const }
    for (const resource of [{ ...base, path: 'https://evil.test/a.png' }, { ...base, path: 'world/test-only/revenge.png' }, { ...base, path: '../avatar.png' }, { ...base, sha256: 'sha256:' + '0'.repeat(64) }]) await expect(result.resources.read(resource, request.signal)).rejects.toThrow()
    expect(calls).toHaveLength(3)
    h.auth.privacy.changePolicy(request.privacyScope, 'block'); await expect(result.resources.read(base, request.signal)).rejects.toThrow(); expect(calls).toHaveLength(3)
  })
  it('pins a caller resource before async work so mutation cannot replace the authorized hash', async () => {
    const h = setup(), request = await requestFor(h.auth)
    let finish!: (value: Response) => void
    const calls: string[] = [], assetFetch: typeof fetch = async input => { calls.push(String(input)); return new Promise(resolve => { finish = resolve }) }
    const result = await createWorldEntryResolver({ ...options(h.auth), assetFetch })(request)
    const doc = request.bootstrap.document as unknown as Bundle, texture = doc.avatar_manifest.atlas.texture
    const resource: { path: string; sha256: string; kind: 'png' } = { path: texture.path!, sha256: texture.sha256!, kind: 'png' }
    const pending = result.resources.read(resource, request.signal), failure = expect(pending).rejects.toThrow()
    await waitFor(() => expect(calls).toHaveLength(1))
    const bad = new TextEncoder().encode('unapproved replacement bytes')
    const { sha256 } = await import('../../../world/map/json')
    resource.sha256 = await sha256(bad)
    finish(assetResponse(calls[0]!, bad)); await failure
  })
  it('aborts an outstanding asset read when the original bootstrap signal ends', async () => {
    const h = setup(), input = await requestFor(h.auth), lifetime = new AbortController(), request = { ...input, signal: lifetime.signal }
    let finish!: (value: Response) => void, fetchSignal: AbortSignal | null = null
    const assetFetch: typeof fetch = async (_input, init) => { fetchSignal = init!.signal!; return new Promise(resolve => { finish = resolve }) }
    const result = await createWorldEntryResolver({ ...options(h.auth), assetFetch })(request)
    const doc = request.bootstrap.document as unknown as Bundle, texture = doc.avatar_manifest.atlas.texture
    const resource: { path: string; sha256: string; kind: 'png' } = { path: texture.path!, sha256: texture.sha256!, kind: 'png' }
    const pending = result.resources.read(resource, new AbortController().signal), failure = expect(pending).rejects.toThrow()
    await waitFor(() => expect(fetchSignal).not.toBeNull()); lifetime.abort(); await failure
    expect((fetchSignal as unknown as AbortSignal).aborted).toBe(true)
    finish(assetResponse(`https://mcr.test/${resource.path}`, assetBytes.png))
    await expect(result.resources.read(resource, new AbortController().signal)).rejects.toThrow()
  })
  it('keeps the validated catalog default even when full; catalog selection is never admission or an invented fallback', async () => {
    const catalog = { ...rooms(), items: rooms().items.map(r => ({ ...r, availability: r.room_id === rooms().default_room_id ? 'FULL' : 'AVAILABLE' })) }
    const h = setup(() => Response.json(catalog)), request = await requestFor(h.auth), result = await createWorldEntryResolver(options(h.auth))(request)
    expect(result.roomId).toBe(catalog.default_room_id); expect(result.rooms.items[0]!.availability).toBe('FULL')
    expect(result).not.toHaveProperty('admitted'); expect(h.requests).toHaveLength(1)
  })
  it.each(['foreign event', 'missing default', 'duplicate rooms'] as const)('refuses %s in the canonical room response', async kind => {
    const data = structuredClone(rooms())
    if (kind === 'foreign event') data.event_id = '00000000-0000-4000-8000-000000000999'
    if (kind === 'missing default') data.default_room_id = '00000000-0000-4000-8000-000000000999'
    if (kind === 'duplicate rooms') data.items = [data.items[0]!, data.items[0]!]
    const h = setup(() => Response.json(data)), request = await requestFor(h.auth)
    await expect(createWorldEntryResolver(options(h.auth))(request)).rejects.toThrow()
  })
  it.each(['map missing', 'theme missing', 'avatar missing', 'synthetic', 'unreviewed', 'foreign avatar', 'external path', 'duplicate path', 'synthetic place'] as const)('rejects %s resources before any room or asset read', async kind => {
    const h = setup(), b = await bundle()
    const theme = b.theme_manifests.find(t => t.theme_id === b.participant.preferences.theme)!
    if (kind === 'map missing') Object.assign(b.map_manifest.tilemap, { resource_state: 'NOT_PROVIDED', path: null, sha256: null })
    if (kind === 'theme missing') Object.assign(theme.texture, { image_state: 'NOT_PROVIDED', path: null, sha256: null })
    if (kind === 'avatar missing') Object.assign(b.avatar_manifest.atlas.texture, { image_state: 'NOT_PROVIDED', path: null, sha256: null })
    if (kind === 'synthetic') b.avatar_manifest.provenance.source_kind = 'ORIGINAL_FIXTURE'
    if (kind === 'unreviewed') Object.assign(b.avatar_manifest.provenance, { review_state: 'UNREVIEWED', reviewed_by: null, reviewed_at: null })
    if (kind === 'foreign avatar') b.avatar_manifest.provenance.source_id = 'legacy_characters'
    if (kind === 'external path') Object.assign(theme.texture, { path: '//evil.test/avatar.png' })
    if (kind === 'duplicate path') Object.assign(theme.texture, { path: b.avatar_manifest.atlas.texture.path })
    if (kind === 'synthetic place') b.map_manifest.places[0]!.provenance.source_kind = 'ORIGINAL_FIXTURE'
    await seal(b)
    const base = await requestFor(h.auth), request = { ...base, bootstrap: { ...base.bootstrap, document: immutable(b) } }, assetFetch = vi.fn<typeof fetch>()
    await expect(createWorldEntryResolver({ ...options(h.auth), assetFetch })(request)).rejects.toThrow()
    expect(h.requests).toHaveLength(0); expect(assetFetch).not.toHaveBeenCalled()
  })
  it('rejects unchanged shipped synthetic bootstrap without inserting provided resources', async () => {
    const h = setup(), scope = { generation: h.auth.session.read().generation, userId, eventId }
    const bootstrap = await decodeBootstrap(original, scope)
    await expect(createWorldEntryResolver(options(h.auth))({ scope, privacyScope: h.auth.privacy.capture(), bootstrap, signal: new AbortController().signal })).rejects.toThrow()
    expect(h.requests).toHaveLength(0)
  })
  it.each(['actor', 'event', 'generation', 'policy', 'projection', 'mutable', 'aborted'] as const)('rejects stale/mismatched %s before reading rooms', async kind => {
    const h = setup(), input = await requestFor(h.auth)
    const request = { ...input }
    if (kind === 'actor') request.scope = { ...input.scope, userId: '00000000-0000-4000-8000-000000000999' }
    if (kind === 'event') request.scope = { ...input.scope, eventId: '00000000-0000-4000-8000-000000000999' }
    if (kind === 'generation') request.scope = { ...input.scope, generation: 99 }
    if (kind === 'policy') h.auth.privacy.changePolicy(input.privacyScope, 'revalidated')
    if (kind === 'projection') request.bootstrap = { ...input.bootstrap, event: { ...input.bootstrap.event, title: 'Tampered title' } }
    if (kind === 'mutable') request.bootstrap = { ...input.bootstrap, document: structuredClone(input.bootstrap.document!) }
    if (kind === 'aborted') request.signal = AbortSignal.abort()
    await expect(createWorldEntryResolver(options(h.auth))(request)).rejects.toThrow(); expect(h.requests).toHaveLength(0)
  })
  it('rejects a frozen accessor document that could change its apparent bytes between reads', async () => {
    const h = setup(), request = await requestFor(h.auth), document = structuredClone(request.bootstrap.document!)
    Object.defineProperty(document, 'server_time', { enumerable: true, get: () => '2026-11-03T00:00:00Z' })
    immutable(document)
    await expect(createWorldEntryResolver(options(h.auth))({ ...request, bootstrap: { ...request.bootstrap, document } })).rejects.toThrow()
    expect(h.requests).toHaveLength(0)
  })
  it('rejects changed manifest hashes and foreign cross-manifest binding', async () => {
    const h = setup(), b = await bundle(); b.map_manifest.geometry.width_cells++
    const request = await requestFor(h.auth)
    await expect(createWorldEntryResolver(options(h.auth))({ ...request, bootstrap: { ...request.bootstrap, document: immutable(b) } })).rejects.toThrow()
    const bad = await bundle(); bad.theme_manifests[0]!.map_revision++
    bad.theme_manifests[0]!.content_hash = await contentHash(bad.theme_manifests[0]!)
    await expect(decodeBootstrap(bad, request.scope)).rejects.toThrow(); expect(h.requests).toHaveLength(0)
  })
  it.each(['abort', 'logout', 'policy', 'identity'] as const)('fences a noncooperative in-flight room response after %s', async kind => {
    let finish!: (value: Response) => void
    const h = setup(request => new URL(request.url).pathname.endsWith('/rooms') ? new Promise(resolve => { finish = resolve }) : new Response(null, { status: 204 }))
    const request = await requestFor(h.auth), external = new AbortController(), pending = createWorldEntryResolver(options(h.auth))({ ...request, signal: external.signal })
    const failure = expect(pending).rejects.toThrow()
    await waitFor(() => expect(h.requests).toHaveLength(1))
    if (kind === 'abort') external.abort()
    if (kind === 'logout') await h.auth.logout()
    if (kind === 'policy') h.auth.privacy.changePolicy(request.privacyScope, 'block')
    if (kind === 'identity') h.auth.session.replace(request.scope.generation, { userId: '00000000-0000-4000-8000-000000000999', eventId, accessToken: 'replacement-token' })
    finish(Response.json(rooms())); await failure
    expect(h.requests[0]!.signal.aborted).toBe(true)
  })
  it('expires through the canonical Auth timer while a room read is outstanding', async () => {
    let finish!: (value: Response) => void
    const h = setup(r => {
      const path = new URL(r.url).pathname
      if (path === '/api/auth/capabilities') return Response.json(capabilities())
      if (path === '/api/auth/login') return Response.json({ ...authResponse(), expiresIn: 1 })
      return new Promise(resolve => { finish = resolve })
    })
    h.auth.start(); await waitFor(() => expect(h.auth.read().emailEnabled).toBe(true))
    expect(await h.auth.login('fixture@example.test', 'fixture-password')).toBe(true)
    const request = await requestFor(h.auth), pending = createWorldEntryResolver(options(h.auth))(request), failure = expect(pending).rejects.toThrow()
    await waitFor(() => expect(h.requests.some(r => new URL(r.url).pathname.endsWith('/rooms'))).toBe(true))
    await new Promise(resolve => setTimeout(resolve, 1050)); finish(Response.json(rooms())); await failure
    expect(h.auth.session.read().userId).toBeNull(); expect(h.auth.read().notice).toBe('expired')
  })
  it('rejects invalid local intent configuration without claiming a C05 movement model', async () => {
    const h = setup(), request = await requestFor(h.auth)
    for (const patch of [{ zoom: 0 }, { zoom: 9 }, { movementPolicy: { ...inputConfiguration.movementPolicy, footWidthPx: 33 } }, { movementPolicy: { ...inputConfiguration.movementPolicy, speedPxPerSecond: Infinity } }]) {
      await expect(createWorldEntryResolver({ ...options(h.auth), localInput: { ...inputConfiguration, ...patch } })(request)).rejects.toThrow()
    }
    expect(() => createWorldEntryResolver({ ...options(h.auth), localInput: { ...inputConfiguration, reviewedBy: '' } })).toThrow()
    expect(h.requests).toHaveLength(0)
  })
})
