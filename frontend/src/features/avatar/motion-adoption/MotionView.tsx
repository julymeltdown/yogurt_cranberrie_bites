import { useLayoutEffect, useRef, useState } from 'react'
import type { MotionRuntime } from '../../../entities/avatar/runtime/motion-adoption/runtime'
import { motionFrame, type MotionDirection } from '../../../entities/avatar/runtime/motion-adoption/art'
import { canonicalJson } from '../../../world/map/json'

/** Profile stays idle; the editor can sample the one existing Phaser clock. Never owns a RAF/timer. */
export function MotionView<C>({ runtime, config, label, size = 64, direction = 'down', walking = false }: {
  readonly runtime: MotionRuntime<C>; readonly config: C; readonly label: string; readonly size?: 56 | 64 | 72 | 192 | 'fill'
  readonly direction?: MotionDirection; readonly walking?: boolean
}) {
  const host = useRef<HTMLCanvasElement>(null), [ready, setReady] = useState(false)
  let key = ''
  try { key = canonicalJson(runtime.validateConfig(config) as Record<string, unknown>) } catch { /* Invalid portrait remains unavailable. */ }
  useLayoutEffect(() => {
    const element = host.current, context = element?.getContext('2d'), controller = new AbortController()
    let release = () => {}, unsubscribe = () => {}, stopClock = () => {}
    const clear = () => { context?.clearRect(0, 0, 64, 64); if (!controller.signal.aborted) setReady(false) }
    clear()
    if (!key || !context || !element) return
    void runtime.acquire(config, { signal: controller.signal }).then(lease => {
      release = lease.dispose
      if (controller.signal.aborted) { lease.dispose(); return }
      unsubscribe = lease.subscribe(clear)
      const draw = (time: number) => {
        const render = lease.read()
        if (!render) { clear(); return }
        const frame = motionFrame(direction, walking, time)
        context.clearRect(0, 0, 64, 64); context.imageSmoothingEnabled = false
        context.drawImage(render.canvas, frame % 5 * 64, Math.floor(frame / 5) * 64, 64, 64, 0, 0, 64, 64)
        element.dataset.frame = String(frame); setReady(true)
      }
      draw(runtime.readTime()); if (walking) stopClock = runtime.onFrame(draw)
    }).catch(() => { if (!controller.signal.aborted) clear() })
    return () => { controller.abort(); stopClock(); unsubscribe(); release(); context.clearRect(0, 0, 64, 64) }
    // key is the validated config projection; object identity must not reload equivalent avatars.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runtime, key, direction, walking])
  return <span data-motion-state={ready && key ? 'ready' : 'unavailable'} style={size === 'fill' ? { display: 'block', width: '100%' } : undefined}>
    <canvas ref={host} width={64} height={64} style={{ ...(size === 'fill' ? { width: '100%', height: 'auto' } : { width: size, height: size }), imageRendering: 'pixelated', display: ready && key ? 'block' : 'none' }} role="img" aria-label={label} />
    {!(ready && key) && <span role="status">Character unavailable</span>}
  </span>
}
