import { useEffect, useState } from 'react'
import type { MotionRuntime } from '../../../entities/avatar/runtime/motion-adoption/runtime'
import type { MotionSelection } from '../../../entities/avatar/runtime/motion-adoption/art'
import { canonicalJson } from '../../../world/map/json'
import { MotionView } from './MotionView'

/** Only the three displayed options acquire previews; all borrow the same scoped art owner. */
export function MotionOption<C>({ runtime, draft, selection, label, pressed, disabled, onChoose }: {
  readonly runtime: MotionRuntime<C>; readonly draft: C; readonly selection: MotionSelection
  readonly label: string; readonly pressed: boolean; readonly disabled: boolean; onChoose(): void
}) {
  const [preview, setPreview] = useState<{ key: string; config: C } | null>(null)
  const key = canonicalJson({ draft, selection })
  useEffect(() => {
    const controller = new AbortController()
    void runtime.choose(draft, selection, controller.signal).then(config => { if (!controller.signal.aborted) setPreview({ key, config }) }).catch(() => { if (!controller.signal.aborted) setPreview(null) })
    return () => { controller.abort() }
    // key includes both validated choices; reference churn must not reacquire.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runtime, key])
  return <button type="button" disabled={disabled} aria-label={`Choose ${label}`} aria-pressed={pressed} onClick={onChoose}>
    {preview?.key === key ? <MotionView runtime={runtime} config={preview.config} size={56} label={`${label} preview`} /> : <span>Loading…</span>}
    <span>{label}</span>
  </button>
}
