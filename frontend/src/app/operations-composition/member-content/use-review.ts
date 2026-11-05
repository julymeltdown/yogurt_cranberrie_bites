import { useEffect, useState } from 'react'
import type { ModerationController } from '../../../features/admin/reports'
import type { MemberContentEntry } from './index'

/** A stale route binding is never rendered while the next effect is pending. */
export function useMemberContentReview(entry: MemberContentEntry | null, target: string | null) {
  const [slot, setSlot] = useState<{ entry: MemberContentEntry; target: string; controller: ModerationController | null } | null>(null)
  useEffect(() => {
    if (entry && target) setSlot({ entry, target, controller: target === 'content' ? entry.content() : entry.member(target) })
    else setSlot(null)
    // The decision owner retains controllers when a route unmounts.
  }, [entry, target])
  const bound = slot?.entry === entry && slot?.target === target
  return { controller: bound && entry?.isCurrent() ? slot?.controller ?? null : null,
    opening: !!entry && !!target && !bound }
}
