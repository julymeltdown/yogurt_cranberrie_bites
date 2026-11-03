import type { ValuePort } from './observer'
export type BrowserNetworkHint = 'online' | 'offline' | 'unknown'
/** Optional root-owned hint adapter. It never tests server availability, reconnects,
 * handles visibility, or changes protected state. Install once beside the observer.
 */
export function createBrowserConnectivity(browser: Pick<Window, 'addEventListener' | 'removeEventListener' | 'navigator'>): ValuePort<BrowserNetworkHint> & { start(): void; dispose(): void } {
  const listeners = new Set<() => void>()
  let started = false, disposed = false, value: BrowserNetworkHint = 'unknown'
  const update = () => {
    if (disposed) return
    const next = browser.navigator.onLine === false ? 'offline' : browser.navigator.onLine === true ? 'online' : 'unknown'
    if (next === value) return
    value = next
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Independent presentation listeners. */ } }
  }
  return { read: () => value,
    subscribe(fn) { if (disposed) return () => {}; listeners.add(fn); return () => { listeners.delete(fn) } },
    start() { if (started || disposed) return; started = true; browser.addEventListener('online', update); browser.addEventListener('offline', update); update() },
    dispose() { if (disposed) return; disposed = true; browser.removeEventListener('online', update); browser.removeEventListener('offline', update); listeners.clear(); value = 'unknown' },
  }
}
