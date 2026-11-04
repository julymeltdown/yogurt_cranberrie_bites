import { MapError } from '../map/json'
import { abortable } from '../map/resources'
import type { RuntimeEffectLease, RuntimeEffects, RuntimeEffectValue } from './contract'
export type RuntimeEffectOwner = Readonly<{ effects: RuntimeEffects; signal: AbortSignal }>
/** A stable public facade; every acquisition captures exactly one ready runtime owner. */
export function createRuntimeEffects(readOwner: () => RuntimeEffectOwner | null, timeoutMs: number): RuntimeEffects {
  let active: AbortController | null = null, revision = 0
  return {
    get motion() { return readOwner()?.effects.motion === true },
    get lowPower() { return readOwner()?.effects.lowPower === true },
    acquire(value: RuntimeEffectValue, signal: AbortSignal) {
      if (!value || typeof value.reducedMotion !== 'boolean' || typeof value.lowPower !== 'boolean') throw new MapError('invalid')
      if (signal.aborted) throw new MapError('cancelled')
      const owner = readOwner()
      if (!owner || owner.signal.aborted || value.reducedMotion && !owner.effects.motion || value.lowPower && !owner.effects.lowPower) throw new MapError('unavailable')
      const ticket = ++revision
      active?.abort()
      if (ticket !== revision) throw new MapError('cancelled')
      const controller = new AbortController(); active = controller
      let lease: RuntimeEffectLease | null = null, released = false
      const release = () => {
        if (released) return
        released = true; controller.abort()
        signal.removeEventListener('abort', release); owner.signal.removeEventListener('abort', release)
        const old = lease; lease = null
        if (active === controller) active = null
        try { old?.dispose() } catch { /* Release the lifetime even if an injected engine is broken. */ }
      }
      signal.addEventListener('abort', release, { once: true }); owner.signal.addEventListener('abort', release, { once: true })
      // Engine effects listen to this signal synchronously, including before an async lease settles.
      controller.signal.addEventListener('abort', release, { once: true })
      const current = () => !released && !controller.signal.aborted && !signal.aborted && !owner.signal.aborted && active === controller && readOwner()?.signal === owner.signal
      let pending: RuntimeEffectLease | Promise<RuntimeEffectLease>
      if (!current()) { release(); throw new MapError('cancelled') }
      try { pending = owner.effects.acquire(Object.freeze({ ...value }), controller.signal) }
      catch (error) { release(); throw error }
      const reserved = Promise.resolve(pending).then(next => {
        if (!current()) { next.dispose(); throw new MapError('cancelled') }
        lease = next; return next
      })
      return abortable(reserved, controller.signal, timeoutMs).then(() => {
        if (!current()) { release(); throw new MapError('cancelled') }
        return { dispose: release }
      }, error => { release(); throw error })
    },
  }
}
