// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { act, cleanup, render, waitFor } from '@testing-library/react'
import { createReviewedAtlasLeases, createReviewedAvatarCompositor } from '../../../../entities/avatar/runtime'
import { decodeImage } from '../../../../world/map/resources'
import type { Resource } from '../../../../world/map/resources'
import { contentHash } from '../../../../world/map/json'
import { createAccountOwner } from '../../owner'
import { AvatarThumbnail } from '../../../../features/me/ui/AvatarThumbnail'
import { ReviewedProfileAvatar } from '../../../../features/profile/Avatar'
import { setup, participant, userId, eventId } from '../../../../features/me/__tests__/fixtures'
import { bootstrapFor } from '../../../__tests__/fixtures/ports'
import { fixture } from './fixture'

vi.mock('../../../../entities/avatar/source-catalog', () => import('./catalog'))
vi.mock('../../../../world/map/resources', async original => ({ ...await original<typeof import('../../../../world/map/resources')>(), decodeImage: vi.fn() }))
const signal = () => new AbortController().signal
const disposed: ReturnType<typeof vi.fn>[] = []
beforeEach(() => {
  vi.mocked(decodeImage).mockImplementation(async () => {
    const dispose = vi.fn(); disposed.push(dispose)
    return { image: { naturalWidth: 320, naturalHeight: 1280 } as HTMLImageElement, dispose }
  })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ drawImage: vi.fn(), imageSmoothingEnabled: true } as unknown as CanvasRenderingContext2D)
})
afterEach(() => { cleanup(); disposed.length = 0; vi.restoreAllMocks(); vi.resetAllMocks() })
async function input() { const f = await fixture(); return { ...f, resources: { read: vi.fn(async (_resource: Resource, _signal: AbortSignal) => f.png) } } }

it('coalesces read/decode, retains warm atlas, keeps consumer canvases independent and scrubs on owner teardown', async () => {
  const source = await input(), leases = createReviewedAtlasLeases({ isCurrent: () => true })
  const [a, b] = await Promise.all([createReviewedAvatarCompositor({ ...source, leases, signal: signal() }), createReviewedAvatarCompositor({ ...source, leases, signal: signal() })])
  expect(source.resources.read).toHaveBeenCalledOnce(); expect(decodeImage).toHaveBeenCalledOnce()
  const pose = { direction: 'down', animation: 'idle', frame: 0 } as const
  const first = a.thumbnail(source.document.presets[0]!.config, pose), second = b.thumbnail(source.document.presets[0]!.config, pose)
  expect(first.canvas).not.toBe(second.canvas); expect(first.canvas.width).toBe(64)
  a.dispose(); expect(first.canvas.width).toBe(0); expect(second.canvas.width).toBe(64)
  b.dispose(); const warm = await createReviewedAvatarCompositor({ ...source, leases, signal: signal() })
  expect(decodeImage).toHaveBeenCalledOnce(); expect(leases.read()).toMatchObject({ entries: 1, references: 1, loads: 1, hits: 2 })
  leases.dispose(); leases.dispose(); expect(warm.read().disposed).toBe(true)
  expect(disposed[0]).toHaveBeenCalledOnce(); expect(leases.read()).toMatchObject({ entries: 0, reservedDecodedBytes: 0, references: 0, disposed: true })
})
it('cancels one waiting consumer without cancelling the shared owner load', async () => {
  const source = await input(), leases = createReviewedAtlasLeases({ isCurrent: () => true }), caller = new AbortController()
  let finish!: (value: Uint8Array<ArrayBuffer>) => void
  source.resources.read.mockImplementation(() => new Promise(resolve => { finish = resolve }))
  const a = leases.acquire(source, caller.signal), rejected = expect(a).rejects.toThrow('cancelled'), b = leases.acquire(source, signal())
  await waitFor(() => expect(source.resources.read).toHaveBeenCalledOnce()); caller.abort(); await rejected
  expect(source.resources.read.mock.calls[0]![1].aborted).toBe(false)
  finish(source.png); const live = await b
  expect(leases.read()).toMatchObject({ references: 1, pending: 0 }); live.dispose(); live.dispose(); leases.dispose()
})
it('does not let aborting the first acquired compositor clear another compositor or the warm atlas', async () => {
  const source = await input(), leases = createReviewedAtlasLeases({ isCurrent: () => true }), caller = new AbortController()
  const a = await createReviewedAvatarCompositor({ ...source, leases, signal: caller.signal })
  const b = await createReviewedAvatarCompositor({ ...source, leases, signal: signal() })
  caller.abort(); expect(a.read().disposed).toBe(true); expect(b.read().disposed).toBe(false)
  expect(leases.read().references).toBe(1); b.dispose(); expect(disposed[0]).not.toHaveBeenCalled(); leases.dispose()
})
it('disposes an ignored-abort late decoded image and rejects all pending consumers', async () => {
  const source = await input(), leases = createReviewedAtlasLeases({ isCurrent: () => true })
  let finish!: (value: Awaited<ReturnType<typeof decodeImage>>) => void
  vi.mocked(decodeImage).mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
  const pending = leases.acquire(source, signal()), rejected = expect(pending).rejects.toThrow('cancelled')
  await waitFor(() => expect(decodeImage).toHaveBeenCalledOnce()); leases.dispose(); await rejected
  const dispose = vi.fn(); finish({ image: { naturalWidth: 320, naturalHeight: 1280 } as HTMLImageElement, dispose })
  await waitFor(() => expect(dispose).toHaveBeenCalledOnce()); expect(leases.read().entries).toBe(0)
})
it('evicts failed loads and allows retry while rejecting corrupted bytes before decode', async () => {
  const source = await input(), leases = createReviewedAtlasLeases({ isCurrent: () => true })
  const broken = source.png.slice(); broken[32] = 7
  source.resources.read.mockResolvedValueOnce(broken)
  await expect(leases.acquire(source, signal())).rejects.toThrow(); expect(decodeImage).not.toHaveBeenCalled(); expect(leases.read().entries).toBe(0)
  vi.mocked(decodeImage).mockRejectedValueOnce(new Error('decode failed'))
  await expect(leases.acquire(source, signal())).rejects.toThrow('decode failed'); expect(leases.read().entries).toBe(0)
  const live = await leases.acquire(source, signal()); expect(leases.read().loads).toBe(3); live.dispose(); leases.dispose()
})
it('revalidates manifest pins and complete binding on warm hits and isolates resource identities', async () => {
  const source = await input(), leases = createReviewedAtlasLeases({ isCurrent: () => true })
  const a = await leases.acquire(source, signal()); a.dispose()
  for (const invalid of [
    { ...source, binding: undefined },
    { ...source, pin: { ...source.pin, manifest_version: 2 } },
    { ...source, binding: { ...source.binding, reviewedBy: '' } },
    { ...source, binding: { ...source.binding, textureHash: `sha256:${'0'.repeat(64)}` } },
    { ...source, binding: { ...source.binding, sources: [{ sourceId: 'not-approved', sha256: source.binding.sources[0]!.sha256 }] } },
    { ...source, document: { ...source.document, manifest_version: 2 } },
  ]) await expect(leases.acquire(invalid as typeof source, signal())).rejects.toThrow()
  expect(decodeImage).toHaveBeenCalledOnce()
  const b = await leases.acquire({ ...source, resources: { read: async () => source.png } }, signal())
  expect(decodeImage).toHaveBeenCalledTimes(2); b.dispose(); leases.dispose()
})
it('bounds reserved decoded bytes, protects live leases and evicts only released images', async () => {
  const source = await input(), size = 320 * 1280 * 4, leases = createReviewedAtlasLeases({ isCurrent: () => true, maxEntries: 1, maxDecodedBytes: size })
  const a = await leases.acquire(source, signal()), changed = { ...source, binding: { ...source.binding, reviewedBy: 'SYNTHETIC_REVIEW_2' } }
  await expect(leases.acquire(changed, signal())).rejects.toMatchObject({ reason: 'capacity' })
  expect(leases.read().reservedDecodedBytes).toBe(size); a.dispose()
  const b = await leases.acquire(changed, signal()); expect(disposed[0]).toHaveBeenCalledOnce()
  b.dispose(); leases.dispose()
  const small = createReviewedAtlasLeases({ isCurrent: () => true, maxDecodedBytes: size - 1 })
  await expect(small.acquire(source, signal())).rejects.toMatchObject({ reason: 'capacity' }); small.dispose()
})
it('checks original owner epoch after async validation and before serving a warm hit', async () => {
  const source = await input(); let current = true
  const leases = createReviewedAtlasLeases({ isCurrent: () => current }), a = await leases.acquire(source, signal()); a.dispose()
  const pending = leases.acquire(source, signal()); current = false
  await expect(pending).rejects.toMatchObject({ reason: 'cancelled' }); expect(disposed[0]).toHaveBeenCalledOnce()
  expect(leases.read()).toMatchObject({ disposed: true, entries: 0 }); expect(source.resources.read).toHaveBeenCalledOnce()
})
it('rejects pre-aborted consumers and disposed owners before any read or decode', async () => {
  const source = await input(), leases = createReviewedAtlasLeases({ isCurrent: () => true }), caller = new AbortController()
  caller.abort(); await expect(leases.acquire(source, caller.signal)).rejects.toMatchObject({ reason: 'cancelled' })
  leases.dispose(); await expect(leases.acquire(source, signal())).rejects.toMatchObject({ reason: 'cancelled' })
  expect(source.resources.read).not.toHaveBeenCalled(); expect(decodeImage).not.toHaveBeenCalled()
})
it('keeps one bounded owner load warm after every waiting caller aborts', async () => {
  const source = await input(), leases = createReviewedAtlasLeases({ isCurrent: () => true }), caller = new AbortController()
  let finish!: (value: Uint8Array<ArrayBuffer>) => void
  source.resources.read.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
  const pending = leases.acquire(source, caller.signal), rejected = expect(pending).rejects.toThrow('cancelled')
  await waitFor(() => expect(source.resources.read).toHaveBeenCalledOnce()); caller.abort(); await rejected
  finish(source.png); await waitFor(() => expect(decodeImage).toHaveBeenCalledOnce())
  const live = await leases.acquire(source, signal()); expect(source.resources.read).toHaveBeenCalledOnce()
  live.dispose(); leases.dispose()
})

async function account() {
  const source = await input(), p = participant(), f = setup(vi.fn(async () => Response.json(p))), scope = f.auth.privacy.capture()
  const bootstrap = { ...bootstrapFor({ userId, eventId, generation: scope.generation }), document: { avatar_manifest: source.document } }
  const owner = createAccountOwner({ auth: f.auth, bootstrap, scope, assets: { binding: source.binding, resources: source.resources } })
  return { ...f, source, scope, owner }
}
it('retains one decoded atlas across thumbnails/editor restarts and existing local drafts, with server reconciliation', async () => {
  const f = await account(), config = participant().profile.avatar.config
  const view = render(<AvatarThumbnail auth={f.auth} config={config} source={f.owner.source!} />)
  await waitFor(() => expect(view.container.querySelector('canvas')?.width).toBe(64)); view.unmount()
  act(() => f.owner.avatar.start()); await waitFor(() => expect(f.owner.avatar.read().phase).toBe('ready'))
  act(() => { f.owner.avatar.selectItem('accessory', null); f.owner.avatar.stop(); f.owner.avatar.start() })
  await waitFor(() => expect(f.owner.avatar.read().phase).toBe('ready'))
  expect(f.owner.avatar.read().draft?.item_ids.accessory).toBeNull()
  expect(f.source.resources.read).toHaveBeenCalledOnce(); expect(decodeImage).toHaveBeenCalledOnce()
  f.owner.dispose(); expect(disposed[0]).toHaveBeenCalledOnce(); f.dispose()
})
it.each(['consent', 'identity', 'event', 'generation'] as const)('scrubs account atlas and all consumer canvases on %s invalidation', async kind => {
  const f = await account(), view = render(<AvatarThumbnail auth={f.auth} config={participant().profile.avatar.config} source={f.owner.source!} />)
  await waitFor(() => expect(view.container.querySelector('canvas')?.width).toBe(64)); const canvas = view.container.querySelector('canvas')!
  act(() => {
    if (kind === 'consent') f.auth.privacy.changePolicy(f.scope, 'consent-revoked')
    else f.auth.session.replace(f.auth.session.read().generation, { userId: kind === 'identity' ? '33333333-3333-4333-8333-333333333333' : userId,
      eventId: kind === 'event' ? '44444444-4444-4444-8444-444444444444' : eventId, accessToken: 'SYNTHETIC_NEXT' })
  })
  expect(f.owner.isDisposed()).toBe(true); expect(canvas.width).toBe(0); expect(disposed[0]).toHaveBeenCalledOnce()
  view.unmount(); f.dispose()
})
it('lets profile borrow the retained source without resolving or stealing Me canvas and rejects an expired source', async () => {
  const f = await account(), resolve = vi.fn(async () => f.owner.source!.options), config = participant().profile.avatar.config
  const view = render(<><AvatarThumbnail auth={f.auth} config={config} source={f.owner.source!} />
    <ReviewedProfileAvatar config={config} label="Synthetic profile" options={{ auth: f.auth, scope: f.scope, resolve, source: f.owner.source! }} /></>)
  await waitFor(() => expect(view.container.querySelectorAll('canvas')).toHaveLength(2))
  expect(resolve).not.toHaveBeenCalled(); expect(decodeImage).toHaveBeenCalledOnce()
  act(() => f.auth.privacy.changePolicy(f.scope, 'block')); expect(view.container.querySelectorAll('canvas')).toHaveLength(0)
  view.unmount(); f.dispose()
})
it('does not attach a new profile privacy epoch to an expired borrowed source', async () => {
  const f = await account(), oldSource = f.owner.source!, resolve = vi.fn(async () => oldSource.options)
  f.auth.privacy.changePolicy(f.scope, 'consent-revoked')
  const current = f.auth.privacy.capture()
  const view = render(<ReviewedProfileAvatar config={participant().profile.avatar.config} label="Synthetic profile"
    options={{ auth: f.auth, scope: current, resolve, source: oldSource }} />)
  await waitFor(() => expect(view.container.textContent).toContain('Avatar unavailable'))
  expect(resolve).not.toHaveBeenCalled(); expect(f.source.resources.read).not.toHaveBeenCalled()
  view.unmount(); f.dispose()
})
it('rejects a changed manifest hash without decoding a warm atlas under a different revision', async () => {
  const source = await input(), leases = createReviewedAtlasLeases({ isCurrent: () => true })
  const a = await leases.acquire(source, signal()); a.dispose()
  const document = structuredClone(source.document); document.provenance.author = 'changed'; document.content_hash = await contentHash(document)
  await expect(leases.acquire({ ...source, document, pin: document }, signal())).rejects.toThrow()
  expect(decodeImage).toHaveBeenCalledOnce(); leases.dispose()
})
