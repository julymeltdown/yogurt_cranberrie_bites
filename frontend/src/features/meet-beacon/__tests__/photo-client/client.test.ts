import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import frozen from './frozen-contract.json'
import { photoHarness } from './harness'
import { file, uid } from '../../../media/model/__tests__/fixtures'
import { deferred, beaconId, sampleReceipt } from '../fixtures/harness'
import { meetingSchema, sameMeeting } from '../../model/contract'
import { decodeMedia } from '../../../../shared/media/contract'
const stops: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }))
afterEach(() => { stops.splice(0).reverse().forEach(stop => stop()); vi.unstubAllGlobals() })
async function setup() { const h = await photoHarness(); stops.push(h.dispose); h.fill(); return h }
async function ready(h: Awaited<ReturnType<typeof setup>>) {
  await h.controller.selectPhoto(file())
  h.controller.setDraft({ photoPermissionAcknowledged: true })
  await h.controller.uploadPhoto()
  expect(h.controller.read().photoDraft?.phase).toBe('ready')
  h.controller.setDraft({ acknowledged: true })
}
it('matches the frozen meeting-photo/v1 fixture and keeps BEACON purpose distinct from POST', () => {
  expect(frozen.contract).toBe('meeting-photo/v1')
  expect(meetingSchema.parse(frozen.create.meeting).media_id).toBe(frozen.media_id)
  expect(frozen.create.photo_permission_acknowledged).toBe(true)
  const scope = { generation: 1, userId: frozen.actor_id, eventId: frozen.event_id }
  expect(decodeMedia(frozen.media, scope, frozen.media_id, 'BEACON').purpose).toBe('BEACON')
  expect(() => decodeMedia(frozen.media, scope, frozen.media_id, 'POST')).toThrow()
  expect(sameMeeting(meetingSchema.parse(frozen.create.meeting), meetingSchema.parse({ ...frozen.create.meeting, media_id: null }))).toBe(false)
})
it('requires separate photo permission, then uploads BEACON and saves/rereads the persisted identifier and authenticated Blob', async () => {
  const h = await setup()
  await h.controller.selectPhoto(file()); await h.controller.uploadPhoto()
  expect(h.requests.filter(r => r.url.endsWith('/uploads'))).toHaveLength(0)
  expect(await h.controller.create()).toBe(false)
  h.controller.setDraft({ photoPermissionAcknowledged: true }); await h.controller.uploadPhoto()
  h.controller.setDraft({ acknowledged: true }); expect(await h.controller.create()).toBe(true)
  await vi.waitFor(() => expect(h.controller.read().photo?.mediaId).toBe(h.mediaId))
  const request = h.requests.find(r => r.method === 'POST' && r.url.endsWith('/beacons'))!
  expect(await request.clone().json()).toMatchObject({ meeting: { media_id: h.mediaId }, physical_presence_acknowledged: true, photo_permission_acknowledged: true })
  expect(h.persisted().meeting.media_id).toBe(h.mediaId)
  expect(h.controller.read().photo?.src).toMatch(/^blob:/)
  const old = h.controller.read().photo!.src
  await h.controller.select(null); expect(h.revoked).toContain(old)
  await h.controller.select(beaconId)
  await vi.waitFor(() => expect(h.controller.read().photo?.mediaId).toBe(h.mediaId))
  expect(h.controller.read().photo?.src).not.toBe(old)
  h.controller.dispose(); const fresh = h.newReader(); stops.push(fresh.dispose); fresh.start(); await fresh.select(beaconId)
  await vi.waitFor(() => expect(fresh.read().photo?.mediaId).toBe(h.mediaId))
  expect(fresh.read().draft.mediaId).toBeNull()
})
it('retaining an attachment requires a fresh acknowledgement; explicit removal persists on the next revision', async () => {
  const h = await setup(); await ready(h); await h.controller.create()
  await vi.waitFor(() => expect(h.controller.read().photo).not.toBeNull())
  expect(h.controller.editCurrent()).toBe(true)
  h.controller.setDraft({ acknowledged: true }); expect(await h.controller.update()).toBe(false)
  h.controller.removePhoto(); h.controller.setDraft({ acknowledged: true })
  expect(await h.controller.update()).toBe(true)
  expect(h.persisted().meeting.media_id ?? null).toBeNull()
  expect(h.controller.read().photo).toBeNull()
  const last = await [...h.requests].reverse().find(r => r.method === 'PATCH')!.clone().json()
  expect(last.expected_revision).toBe(1); expect(last.meeting.media_id).toBeUndefined(); expect(last.photo_permission_acknowledged).toBeUndefined()
})
it('does not attach failed/unknown uploads or replay mutations during explicit receipt recovery', async () => {
  const h = await setup(); h.finalizeMode('lost'); await h.controller.selectPhoto(file())
  h.controller.setDraft({ photoPermissionAcknowledged: true, acknowledged: true }); await h.controller.uploadPhoto()
  expect(h.controller.read().photoDraft?.phase).toBe('uncertain'); expect(await h.controller.create()).toBe(false)
  await h.controller.select(beaconId)
  expect(h.controller.read().photoDraft?.phase).toBe('uncertain')
  const last = h.requests.find(r => r.url.endsWith('/finalize'))!, key = last.headers.get('idempotency-key')!
  h.setStatus({ ...sampleReceipt('finalizeConcertUpload', 'SUCCEEDED'), operation_id: 'finalizeConcertUpload', idempotency_key: key, resource_id: h.mediaId, resource_revision: 3 })
  await h.controller.checkPhotoUpload()
  expect(h.controller.read().photoDraft?.phase).toBe('ready')
  expect(h.requests.filter(r => r.url.endsWith('/finalize'))).toHaveLength(1)
  expect(new URL([...h.requests].reverse().find(r => new URL(r.url).pathname.endsWith('/commands'))!.url).searchParams.get('idempotency_key')).toBe(key)
})
it.each(['expired', 'revoked', 'denied', 'policy', 'block', 'account', 'event'] as const)('clears saved photo content and owned URLs for %s', async mode => {
  const h = await setup(); await ready(h); await h.controller.create()
  await vi.waitFor(() => expect(h.controller.read().photo).not.toBeNull()); const url = h.controller.read().photo!.src
  if (mode === 'expired') h.setTime(Date.parse(h.persisted().meeting.expires_at))
  if (mode === 'revoked') h.controller.invalidate({ beaconId, revision: 2, state: 'REVOKED' })
  if (mode === 'denied') { h.denyReads(); await h.controller.refresh(); await vi.waitFor(() => expect(h.controller.read().photoLoading).toBe(false)) }
  if (mode === 'policy') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'suspended')
  if (mode === 'block') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block')
  if (mode === 'account') h.auth.session.replace(h.auth.session.read().generation, { userId: uid(999), eventId: h.auth.session.read().eventId!, accessToken: 'OTHER_SYNTHETIC_USER' })
  if (mode === 'event') h.auth.session.replace(h.auth.session.read().generation, { userId: h.auth.session.read().userId!, eventId: uid(999), accessToken: 'OTHER_SYNTHETIC_EVENT' })
  expect(h.controller.read().photo).toBeNull(); expect(h.revoked).toContain(url)
  if (mode === 'denied') {
    h.denyReads(false); await h.controller.refresh()
    await vi.waitFor(() => expect(h.controller.read().photo?.mediaId).toBe(h.mediaId))
    expect(h.controller.read().photo!.src).not.toBe(url)
  }
})
it('rejects late protected bytes after revision invalidation and ignores direct untrusted media-id draft injection', async () => {
  const h = await setup(), gate = deferred<Response>(); await ready(h); h.gateContent(gate.promise)
  await h.controller.create(); await vi.waitFor(() => expect(h.controller.read().photoLoading).toBe(true))
  h.controller.invalidate({ beaconId, revision: 2 }); const count = h.created.length
  gate.resolve(new Response(new Uint8Array([1, 2]).buffer, { headers: { 'Content-Type': 'image/png' } }))
  await new Promise(resolve => setTimeout(resolve, 0))
  expect(h.created).toHaveLength(count); expect(h.controller.read().photo).toBeNull()
  h.controller.setDraft(Object.assign({ acknowledged: true }, { mediaId: uid(999) }))
  expect(h.controller.read().draft.mediaId).toBeNull()
})
it('replaces an attachment with the new READY identifier and clears the earlier revision URL', async () => {
  const h = await setup(); await ready(h); await h.controller.create()
  await vi.waitFor(() => expect(h.controller.read().photo).not.toBeNull()); const old = h.controller.read().photo!.src
  h.controller.editCurrent(); await ready(h)
  const replacement = h.controller.read().draft.mediaId!
  expect(replacement).not.toBe(h.mediaId)
  expect(await h.controller.update()).toBe(true)
  await vi.waitFor(() => expect(h.controller.read().photo?.mediaId).toBe(replacement))
  expect(h.persisted().revision).toBe(2); expect(h.revoked).toContain(old)
})
it('cancels local selection without uploading, and rejects a definitive finalize failure as an attachment', async () => {
  const h = await setup(); await h.controller.selectPhoto(file()); const preview = h.controller.read().photoDraft!.previewUrl!
  h.controller.removePhoto(); expect(h.revoked).toContain(preview)
  expect(h.requests.some(r => r.url.endsWith('/uploads'))).toBe(false)
  h.finalizeMode('failed'); await h.controller.selectPhoto(file())
  h.controller.setDraft({ photoPermissionAcknowledged: true, acknowledged: true }); await h.controller.uploadPhoto()
  expect(h.controller.read().photoDraft?.phase).toBe('failed'); expect(h.controller.read().draft.mediaId).toBeNull()
  expect(await h.controller.create()).toBe(false)
})
it('recovers an unknown beacon save by its original receipt and fresh persisted read, without replay', async () => {
  const h = await setup(); await ready(h); h.setMode('lost')
  expect(await h.controller.create()).toBe(false); expect(h.controller.read().command.phase).toBe('uncertain')
  const post = h.requests.find(r => r.method === 'POST' && r.url.endsWith('/beacons'))!, body = await post.clone().json()
  // Model a committed server write whose response was lost; only later GET reveals it.
  h.setBeacon({ ...h.persisted(), meeting: body.meeting, revision: 1 }); h.setMode('ok')
  h.setStatus(sampleReceipt('createConcertBeacon', 'SUCCEEDED')); await h.controller.checkCommand()
  await vi.waitFor(() => expect(h.controller.read().photo?.mediaId).toBe(h.mediaId))
  expect(h.requests.filter(r => r.method === 'POST' && r.url.endsWith('/beacons'))).toHaveLength(1)
})
it('resets attachment permission when its audience/expiry changes and cancels previews if place permission is withdrawn', async () => {
  const h = await setup(); await ready(h)
  h.controller.setDraft({ ttlSeconds: 60 })
  expect(h.controller.read().draft.photoPermissionAcknowledged).toBe(false)
  expect(await h.controller.create()).toBe(false)
  const url = h.controller.read().photoDraft!.previewUrl!
  h.items[1]!.provenance.photo_permission = 'DENIED'; await h.placesController.refresh(); h.controller.refreshTime()
  expect(h.controller.read().photoDraft?.phase).toBe('cancelled')
  expect(h.controller.read().draft.mediaId).toBeNull(); expect(h.revoked).toContain(url)
})
