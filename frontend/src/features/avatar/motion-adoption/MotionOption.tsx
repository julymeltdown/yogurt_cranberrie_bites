import { useEffect, useState } from 'react'
import type { MotionPart, MotionRuntime } from '../../../entities/avatar/runtime/motion-adoption/runtime'
import type { MotionSelection } from '../../../entities/avatar/runtime/motion-adoption/art'
import { canonicalJson } from '../../../world/map/json'
import { MotionView } from './MotionView'
import { MotionPartPreview } from './part-preview/MotionPartPreview'

/** Only the three displayed options acquire previews; all borrow the same scoped art owner. */
export function MotionOption<C>({ runtime, draft, selection, part, label, pressed, disabled, onChoose }: {
  readonly runtime: MotionRuntime<C>; readonly draft: C; readonly selection: MotionSelection
  readonly part?: MotionPart
  readonly label: string; readonly pressed: boolean; readonly disabled: boolean; onChoose(): void
}) {
  const [preview, setPreview] = useState<{ key: string; config: C } | null>(null)
  const key = canonicalJson({ draft, selection })
  const partKey = part ? canonicalJson(part) : ''
  useEffect(() => {
    if (partKey) return
    const controller = new AbortController()
    void runtime.choose(draft, selection, controller.signal).then(config => { if (!controller.signal.aborted) setPreview({ key, config }) }).catch(() => { if (!controller.signal.aborted) setPreview(null) })
    return () => { controller.abort() }
    // key includes both validated choices; reference churn must not reacquire.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runtime, key, partKey])
  return <button type="button" disabled={disabled} aria-label={`Choose ${label}`} aria-pressed={pressed} onClick={onChoose}>
    {part ? <MotionPartPreview runtime={runtime} part={part} label={`${label} preview`} />
      : preview?.key === key ? <MotionView runtime={runtime} config={preview.config} size={56} label={`${label} preview`} /> : <span>Loading…</span>}
    <span>{label}</span>
  </button>
}
