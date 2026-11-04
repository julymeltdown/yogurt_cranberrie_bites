import { afterEach, expect, it, vi } from 'vitest'
import { createSessionController } from '../../auth/model/session'
import { isolatedCookiePolicy } from '../../auth/__tests__/fixtures/data'
import { file, image, deferred, user, event } from '../../media/model/__tests__/fixtures'
import { createMeetingPhotoScope } from '../model/photo-scope'
const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).reverse().forEach(stop => stop()) })
function fixture() {
  const auth = createSessionController({ eventId: event, cookiePolicy: isolatedCookiePolicy() })
  auth.session.replace(0, { userId: user, eventId: event, accessToken: 'SYNTHETIC_PHOTO_SCOPE' })
  stops.push(() => auth.dispose()); return auth
}
it('uses the existing preparation seam with full captured privacy ownership and disposes its Blob previews', async () => {
  const auth = fixture(), previews = { create: vi.fn(() => 'blob:private-photo'), revoke: vi.fn() }
  const prepare = vi.fn(async () => image()), owner = createMeetingPhotoScope({ auth, usable: () => true, prepare, previews })
  stops.push(owner.dispose)
  const result = await owner.prepare(file(), { signal: new AbortController().signal, onProgress() {} })
  expect(result).toEqual(image()); expect(prepare).toHaveBeenCalledTimes(1)
  const url = owner.previews.create(result.blob)
  auth.privacy.changePolicy(owner.scope, 'block')
  expect(owner.session.read().signal.aborted).toBe(true)
  expect(owner.session.isCurrent(auth.session.read())).toBe(false)
  expect(owner.isCurrent()).toBe(false)
  expect(previews.revoke).toHaveBeenCalledExactlyOnceWith(url)
  expect(() => owner.previews.create(result.blob)).toThrow()
  owner.dispose(); expect(previews.revoke).toHaveBeenCalledTimes(1)
})
it('rejects ignored-abort preparation completions and suppresses late progress after policy withdrawal', async () => {
  const auth = fixture(), gate = deferred<ReturnType<typeof image>>(), progress = vi.fn()
  let report: ((value: number) => void) | undefined, signal: AbortSignal | undefined
  const owner = createMeetingPhotoScope({ auth, usable: () => true, prepare: async (_file, options) => {
    report = options.onProgress; signal = options.signal; return gate.promise
  } }); stops.push(owner.dispose)
  const pending = owner.prepare(file(), { signal: new AbortController().signal, onProgress: progress })
  await vi.waitFor(() => expect(signal).toBeDefined())
  auth.privacy.changePolicy(owner.scope, 'suspended'); expect(signal!.aborted).toBe(true)
  report!(90); gate.resolve(image())
  await expect(pending).rejects.toThrow(); expect(progress).not.toHaveBeenCalled()
})
it('fences changing write authority and reentrant preview invalidation without publishing a URL', () => {
  const auth = fixture(); let allowed = true
  const previews = { create: vi.fn(() => { allowed = false; return 'blob:late' }), revoke: vi.fn() }
  const owner = createMeetingPhotoScope({ auth, usable: () => allowed, previews }); stops.push(owner.dispose)
  expect(() => owner.previews.create(image().blob)).toThrow()
  expect(previews.revoke).toHaveBeenCalledExactlyOnceWith('blob:late')
  expect(owner.session.isCurrent(auth.session.read())).toBe(false)
})
