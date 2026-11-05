import { useLayoutEffect, useRef, useState } from 'react'
import type { MotionPart, MotionPartLease, MotionRuntime } from '../../../../entities/avatar/runtime/motion-adoption/runtime'

/** Attach the borrowed canvas itself: no second bitmap, clock or art owner. */
export function MotionPartPreview<C>({ runtime, part, label }: {
  readonly runtime: MotionRuntime<C>; readonly part: MotionPart; readonly label: string
}) {
  const host = useRef<HTMLSpanElement>(null)
  const [phase, setPhase] = useState<'loading' | 'ready' | 'unavailable'>('loading')
  const { kind, key } = part
  useLayoutEffect(() => {
    const target = host.current!, controller = new AbortController()
    let lease: MotionPartLease | null = null, unsubscribe = () => {}
    function clear() { target.replaceChildren() }
    clear(); setPhase('loading')
    void runtime.acquirePartPreview({ kind, key }, { signal: controller.signal }).then(value => {
      if (controller.signal.aborted) { value.dispose(); return }
      lease = value
      const snapshot = value.read()
      if (!snapshot) { value.dispose(); setPhase('unavailable'); return }
      const canvas = snapshot.canvas
      canvas.style.width = canvas.style.height = '56px'; canvas.style.imageRendering = 'pixelated'
      canvas.setAttribute('aria-hidden', 'true')
      target.replaceChildren(canvas)
      unsubscribe = value.subscribe(() => { clear(); setPhase('unavailable') })
      setPhase('ready')
    }).catch(() => { if (!controller.signal.aborted) { clear(); setPhase('unavailable') } })
    return () => { controller.abort(); unsubscribe(); lease?.dispose(); lease = null; clear() }
  }, [runtime, kind, key])
  return <span data-motion-part-state={phase}>
    <span ref={host} role="img" aria-label={label} />
    {phase !== 'ready' && <span>{phase === 'loading' ? 'Loading…' : 'Part unavailable'}</span>}
  </span>
}
