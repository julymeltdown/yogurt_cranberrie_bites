import type { MeetingBeaconOptions } from './ports'
import type { SessionPort } from '../../../shared/api/http'
import { ImageError } from '../../../shared/media/image-policy'
import { prepareImage, type PrepareImage } from '../../../shared/media/prepare-image'
import { browserPreviews, type PreviewPort } from '../../../shared/media/preview'
import type { MediaRequest } from '../../../shared/media/transport'

/** Adapts existing FE04 preparation/transport/preview seams to one full policy
 * epoch. It creates no identity, upload purpose, attachment or HTTP contract.
 * Construct a new adapter after authority changes; an old one never recaptures.
 */
export function createMeetingPhotoScope(options: {
  readonly auth: MeetingBeaconOptions['auth']
  readonly usable: () => boolean
  readonly prepare?: PrepareImage
  readonly previews?: PreviewPort
}) {
  const { auth } = options, scope = auth.privacy.capture(), abort = new AbortController()
  const urls = new Set<string>(), previews = options.previews ?? browserPreviews
  const current = () => !abort.signal.aborted && auth.privacy.isCurrent(scope) && options.usable()
  const assert = () => { if (!current()) throw new ImageError('aborted') }
  function clear() {
    abort.abort()
    for (const url of urls) { urls.delete(url); try { previews.revoke(url) } catch { /* Clear every private preview. */ } }
  }
  assert()
  const stopPrivacy = auth.privacy.registerCleanup(clear)
  const session: SessionPort = { ...auth.session,
    read: () => ({ ...auth.session.read(), signal: abort.signal }),
    isCurrent: expected => current() && auth.session.isCurrent(expected),
  }
  const request: MediaRequest = input => {
    assert()
    return auth.privacy.run(scope, async ({ signal }) => {
      assert()
      const result = await auth.request({ ...input, signal }); assert(); return result
    }, input.signal ? AbortSignal.any([input.signal, abort.signal]) : abort.signal)
  }
  const prepare: PrepareImage = (file, input) => {
    assert()
    return auth.privacy.run(scope, async ({ signal }) => {
      const result = await (options.prepare ?? prepareImage)(file, {
        signal, onProgress: value => { if (current() && !input.signal.aborted) input.onProgress(value) },
      }); assert(); return result
    }, AbortSignal.any([input.signal, abort.signal]))
  }
  const ownedPreviews: PreviewPort = {
    create(blob) {
      assert()
      const url = previews.create(blob)
      if (!current()) { previews.revoke(url); throw new ImageError('aborted') }
      if (!url.startsWith('blob:')) { previews.revoke(url); throw new ImageError('decode-failed') }
      urls.add(url); return url
    },
    revoke(url) { if (urls.delete(url)) previews.revoke(url) },
  }
  return { scope, session, request, prepare, previews: ownedPreviews, isCurrent: current,
    dispose() { clear(); stopPrivacy() },
  }
}
