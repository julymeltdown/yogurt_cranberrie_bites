import { createImageUploadController, createMediaUploadTransport } from '../../media/model/upload-controller'
import { createMediaPreviewController } from '../../../shared/media/read-media'
import type { PrepareImage } from '../../../shared/media/prepare-image'
import type { PreviewPort } from '../../../shared/media/preview'
import type { MeetingBeaconOptions } from './ports'
import { createMeetingPhotoScope } from './photo-scope'

/** Explicit consumer contract opt-in, separate from server/readiness authority.
 * Omit in root composition until its actual producer declares this version.
 */
export interface MeetingPhotoConfiguration {
  readonly contract: 'meeting-photo/v1'
  readonly prepare?: PrepareImage
  readonly previews?: PreviewPort
  readonly newKey?: () => string
}
export function createMeetingPhotoClient(auth: MeetingBeaconOptions['auth'], usable: () => boolean,
  config: MeetingPhotoConfiguration, now: () => number) {
  if (config.contract !== 'meeting-photo/v1') throw new Error('Unsupported meeting photo contract')
  const fixed = createMeetingPhotoScope({ auth, usable,
    ...(config.prepare ? { prepare: config.prepare } : {}), ...(config.previews ? { previews: config.previews } : {}) })
  const upload = createImageUploadController({ session: fixed.session, transport: createMediaUploadTransport(fixed.request),
    purpose: 'BEACON', prepare: fixed.prepare, previews: fixed.previews, now,
    ...(config.newKey ? { key: config.newKey } : {}) })
  const readers = new Set<ReturnType<typeof createMediaPreviewController>>()
  return { scope: fixed.scope, upload, isCurrent: fixed.isCurrent,
    preview(mediaId: string) {
      const reader = createMediaPreviewController({ session: fixed.session, request: fixed.request, mediaId, purpose: 'BEACON', previews: fixed.previews })
      readers.add(reader)
      return { ...reader, dispose() { reader.dispose(); readers.delete(reader) } }
    },
    dispose() { upload.dispose(); for (const reader of readers) reader.dispose(); readers.clear(); fixed.dispose() },
  }
}
export type MeetingPhotoClient = ReturnType<typeof createMeetingPhotoClient>
