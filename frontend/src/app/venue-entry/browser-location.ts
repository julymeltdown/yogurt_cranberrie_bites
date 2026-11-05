import type { GeoFix, PhoneLocationPort, PhoneStatus } from '../../features/paradise-city-map/ports'

/** No geolocation call, permission query or watch is performed during construction. */
export function createBrowserLocation({ signal, geolocation, document: page = globalThis.document, window: host = globalThis.window }: {
  readonly signal: AbortSignal; readonly geolocation?: Pick<Geolocation, 'watchPosition' | 'clearWatch'> | null
  readonly document?: Document; readonly window?: Window
}): PhoneLocationPort {
  const source = geolocation === undefined ? globalThis.navigator?.geolocation : geolocation
  const listeners = new Set<() => void>()
  let status: PhoneStatus = 'idle', watch: number | null = null, generation = 0, disposed = false
  let withdraw: ((reason: 'background' | 'denied' | 'unavailable') => void) | null = null
  function publish(value: PhoneStatus) { status = value; for (const listener of listeners) listener() }
  function clear() { generation++; if (watch !== null) { const id = watch; watch = null; try { source?.clearWatch(id) } catch { /* Still invalidate callbacks. */ } } }
  function stop() { clear(); withdraw = null; if (status === 'watching' || status === 'requesting') publish('paused') }
  function background() {
    if (watch === null && status !== 'requesting') return
    const callback = withdraw; stop(); callback?.('background')
  }
  const visibility = () => { if (page?.visibilityState === 'hidden') background() }
  function dispose() {
    if (disposed) return
    disposed = true; stop(); signal.removeEventListener('abort', dispose)
    page?.removeEventListener('visibilitychange', visibility); host?.removeEventListener('pagehide', background)
    listeners.clear()
  }
  page?.addEventListener('visibilitychange', visibility); host?.addEventListener('pagehide', background)
  signal.addEventListener('abort', dispose, { once: true })
  if (signal.aborted) dispose()
  return {
    read: () => status,
    subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } }, stop, dispose,
    start(onFix: (fix: GeoFix) => void, onWithdraw) {
      stop()
      if (disposed || signal.aborted || page?.visibilityState === 'hidden') { publish('unavailable'); onWithdraw('unavailable'); return }
      if (!source) { publish('unavailable'); onWithdraw('unavailable'); return }
      const epoch = ++generation; withdraw = onWithdraw; publish('requesting')
      try {
        const id = source.watchPosition(position => {
          if (disposed || signal.aborted || epoch !== generation || page?.visibilityState === 'hidden') return
          const { latitude, longitude, accuracy } = position.coords, capturedAtMs = position.timestamp
          if (![latitude, longitude, accuracy, capturedAtMs].every(Number.isFinite) || Math.abs(latitude) > 90 || Math.abs(longitude) > 180 || accuracy < 0) {
            const callback = withdraw; clear(); publish('unavailable'); callback?.('unavailable'); return
          }
          publish('watching'); onFix({ latitude, longitude, accuracy, capturedAtMs })
        }, error => {
          if (disposed || epoch !== generation) return
          const callback = withdraw; clear(); publish(error.code === 1 ? 'denied' : 'unavailable'); callback?.(error.code === 1 ? 'denied' : 'unavailable')
        }, { enableHighAccuracy: false, maximumAge: 0, timeout: 15000 })
        // Also handle synchronous callbacks in test adapters / unusual browser bindings.
        if (epoch === generation && !disposed) watch = id
        else source.clearWatch(id)
      } catch { clear(); publish('unavailable'); onWithdraw('unavailable') }
    },
  }
}
