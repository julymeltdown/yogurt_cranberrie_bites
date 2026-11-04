import type Phaser from 'phaser'
import { MapError } from '../map/json'
import type { RuntimeEffects, RuntimeEffectLease, RuntimeEffectValue } from './contract'
type Renderer = Pick<Phaser.Renderer.Canvas.CanvasRenderer, 'preRender' | 'render' | 'postRender'>
type Tweens = { readonly timeScale: number; setGlobalTimeScale(value: number): unknown }
/** Only paints are throttled. Scene updates and network/authoritative time never pass through this gate. */
export function createEnginePreferenceEffects(renderer: Renderer, tweens: Tweens, now = () => performance.now()) {
  let disposed = false, revision = 0, active: (RuntimeEffectLease & { value: RuntimeEffectValue }) | null = null
  let attempts = 0, paints = 0
  const effects = {
    motion: true, lowPower: true,
    acquire(value: RuntimeEffectValue, signal: AbortSignal): RuntimeEffectLease {
      if (!value || typeof value.reducedMotion !== 'boolean' || typeof value.lowPower !== 'boolean') throw new MapError('invalid')
      if (disposed || signal.aborted) throw new MapError('cancelled')
      const ticket = ++revision
      active?.dispose()
      if (disposed || signal.aborted || ticket !== revision) throw new MapError('cancelled')
      const original = { preRender: renderer.preRender, render: renderer.render, postRender: renderer.postRender }
      const scale = tweens.timeScale
      let released = false, paint = true, lastPaint: number | null = null
      const preRender: Renderer['preRender'] = function(this: Renderer, ...args) {
        const time = now(); attempts++
        paint = lastPaint === null || time - lastPaint >= 1000 / 15
        if (paint) { lastPaint = time; paints++; return original.preRender.apply(this, args) }
      }
      const render: Renderer['render'] = function(this: Renderer, ...args) { if (paint) return original.render.apply(this, args) }
      const postRender: Renderer['postRender'] = function(this: Renderer, ...args) { if (paint) return original.postRender.apply(this, args) }
      const lease = { value: Object.freeze({ ...value }), dispose() {
        if (released) return
        released = true; signal.removeEventListener('abort', lease.dispose)
        if (active === lease) active = null
        // Another owner may replace a method. Only undo exactly the wrappers we installed.
        if (renderer.preRender === preRender) renderer.preRender = original.preRender
        if (renderer.render === render) renderer.render = original.render
        if (renderer.postRender === postRender) renderer.postRender = original.postRender
        if (value.reducedMotion && tweens.timeScale === 0) tweens.setGlobalTimeScale(scale)
      } }
      active = lease
      signal.addEventListener('abort', lease.dispose, { once: true })
      try {
        if (value.lowPower) { renderer.preRender = preRender; renderer.render = render; renderer.postRender = postRender }
        if (value.reducedMotion) tweens.setGlobalTimeScale(0)
        if (signal.aborted || active !== lease || disposed) { lease.dispose(); throw new MapError('cancelled') }
        return lease
      } catch (error) { lease.dispose(); throw error }
    },
  }
  return Object.assign(effects satisfies RuntimeEffects, {
    read: () => Object.freeze({ reducedMotion: active?.value.reducedMotion ?? false, lowPower: active?.value.lowPower ?? false,
      paintRateLimit: active?.value.lowPower ? 15 : null, attempts, paints, disposed }),
    dispose() { if (disposed) return; disposed = true; ++revision; active?.dispose() },
  })
}
export type EnginePreferenceEffects = ReturnType<typeof createEnginePreferenceEffects>
