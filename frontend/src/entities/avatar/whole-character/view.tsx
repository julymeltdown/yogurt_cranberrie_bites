import { useLayoutEffect, useRef, useState } from 'react'
import { validateWholeCharacterConfig } from './contract'
import type { WholeCharacterRuntime } from './runtime'

/** Caller supplies the real admitted identity; no preset/default/legacy substitution. */
export function WholeCharacterView({ runtime, config, label, size = 64, detail = false, className = '' }: {
  readonly runtime: WholeCharacterRuntime; readonly config: unknown; readonly label: string
  readonly size?: 64 | 128 | 256; readonly detail?: boolean; readonly className?: string
}) {
  const canvas = useRef<HTMLCanvasElement>(null), [ready, setReady] = useState(false)
  let key = ''
  try { const c = validateWholeCharacterConfig(config); key = `${c.manifest_id}:${c.manifest_version}:${c.preset_id}` } catch { /* Invalid identity remains absent. */ }
  useLayoutEffect(() => {
    const element = canvas.current, context = element?.getContext('2d'), controller = new AbortController()
    let release = () => {}, unsubscribe = () => {}
    const clear = () => { context?.clearRect(0, 0, size, size); if (!controller.signal.aborted) setReady(false) }
    clear()
    if (!key || !element || !context || ![64, 128, 256].includes(size)) return
    void runtime.acquire(config, { signal: controller.signal, detail }).then(lease => {
      release = lease.dispose
      if (controller.signal.aborted) { lease.dispose(); return }
      unsubscribe = lease.subscribe(clear)
      const render = lease.read()
      if (!render) { clear(); return }
      context.imageSmoothingEnabled = false
      context.drawImage(render.image, render.source.x, render.source.y, render.source.width, render.source.height, 0, 0, size, size)
      setReady(true)
    }).catch(() => { if (!controller.signal.aborted) clear() })
    return () => { controller.abort(); unsubscribe(); release(); context.clearRect(0, 0, size, size) }
    // key is the validated immutable identity projection; new equivalent objects do not reload.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runtime, key, size, detail])
  return <span className={className} data-whole-character-state={ready && key ? 'ready' : 'unavailable'}>
    <canvas ref={canvas} width={size} height={size} style={{ width: size, height: size, imageRendering: 'pixelated', display: ready && key ? 'block' : 'none' }} role="img" aria-label={label} />
    {!(ready && key) && <span role="status">Character unavailable</span>}
  </span>
}
