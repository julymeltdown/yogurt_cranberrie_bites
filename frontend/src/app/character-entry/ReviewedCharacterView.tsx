import type { ReviewedCharacterOwner } from './reviewed-registry'
import { MotionView } from '../../features/avatar/motion-adoption/MotionView'
import { PortraitFrame } from '../../features/avatar/parade-ink/PortraitFrame'

/** Decode at the family boundary; a stale or legacy profile cannot crash the shell or select a fallback preset. */
export function ReviewedCharacterView({ owner, config, label, size }: {
  readonly owner: ReviewedCharacterOwner; readonly config: unknown; readonly label: string; readonly size: 64 | 128
}) {
  try {
    if (owner.isCurrent()) return <PortraitFrame label={label} size={size}><MotionView runtime={owner.runtime} config={owner.authority.decodeConfig(config)} label={label} size={size} /></PortraitFrame>
  } catch { /* A different family is honestly unavailable. */ }
  return <PortraitFrame label={label} size={size}><span role="img" aria-label={`${label} unavailable`}>Character unavailable</span></PortraitFrame>
}
