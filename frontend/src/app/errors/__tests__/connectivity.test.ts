// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import { createBrowserConnectivity } from '../connectivity'
it('observes online/offline only, starts once and completely releases listeners', () => {
  const add = vi.spyOn(window, 'addEventListener'), remove = vi.spyOn(window, 'removeEventListener')
  const hint = vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(true), notify = vi.fn()
  const connectivity = createBrowserConnectivity(window), stop = connectivity.subscribe(notify)
  try {
    expect(connectivity.read()).toBe('unknown'); connectivity.start(); connectivity.start(); expect(connectivity.read()).toBe('online')
    hint.mockReturnValue(false); window.dispatchEvent(new Event('offline')); expect(connectivity.read()).toBe('offline')
    hint.mockReturnValue(true); window.dispatchEvent(new Event('online')); expect(connectivity.read()).toBe('online')
    expect(add.mock.calls.filter(c => ['online', 'offline'].includes(c[0])).map(c => c[0])).toEqual(['online', 'offline'])
    expect(add.mock.calls.some(c => c[0] === 'visibilitychange')).toBe(false)
    stop(); connectivity.dispose(); connectivity.dispose(); const count = notify.mock.calls.length
    window.dispatchEvent(new Event('offline')); expect(notify).toHaveBeenCalledTimes(count); expect(connectivity.read()).toBe('unknown')
    expect(remove.mock.calls.filter(c => ['online', 'offline'].includes(c[0])).map(c => c[0])).toEqual(['online', 'offline'])
  } finally { connectivity.dispose(); vi.restoreAllMocks() }
})
