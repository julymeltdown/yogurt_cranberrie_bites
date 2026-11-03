import type { AssetResources } from '../map/resources'
import { createReviewedAvatarCompositor, type ReviewedAvatarBinding } from '../../entities/avatar/runtime'
import { createAvatarRenderer } from './renderer'

export type { ReviewedAvatarBinding } from '../../entities/avatar/runtime/resources'
/** No product atlas/binding is bundled. Missing or unreviewed art fails unavailable. */
export function createReviewedAvatarRenderer(options: { binding?: ReviewedAvatarBinding; resources: AssetResources; walkFrameMs?: number }) {
  return createAvatarRenderer({ ...(options.walkFrameMs !== undefined ? { walkFrameMs: options.walkFrameMs } : {}), async load(document, signal) {
    return createReviewedAvatarCompositor({ document, pin: document as { manifest_id: string; manifest_version: number; content_hash: string },
      ...(options.binding ? { binding: options.binding } : {}), resources: options.resources, signal })
  } })
}
