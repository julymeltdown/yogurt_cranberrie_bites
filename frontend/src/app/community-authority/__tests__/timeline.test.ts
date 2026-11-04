import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createAuthorityTimeline, createBrowserAuthorityTimeline } from '..'
const stops: (() => void)[] = []
beforeEach(() => vi.useFakeTimers())
afterEach(() => { stops.splice(0).reverse().forEach(fn => fn()); vi.unstubAllGlobals(); vi.useRealTimers() })
it('captures immutable local observations and rejects copies without revoking another borrower', () => {
  let ms = 10
  const t = createAuthorityTimeline({ observe: () => ({ nowMs: ms, origin: 100 }), maxObservationGapMs: 1000, onInterruption: () => () => {} })
  stops.push(t.dispose); const start = t.capture()!; ms = 20; const receipt = t.capture()!; ms = 50
  expect(Object.isFrozen(start)).toBe(true); expect(t.measure(start, receipt)).toEqual({ roundTripMs: 10, sinceStartMs: 40, sinceReceiptMs: 30 })
  expect(t.measure({ ...start }, receipt)).toBeNull(); expect(t.measure(start, receipt)).not.toBeNull()
})
it('watchdog detects discontinuity without a consumer read and stops permanently', () => {
  let ms = 10
  const t = createAuthorityTimeline({ observe: () => ({ nowMs: ms, origin: 100 }), maxObservationGapMs: 1000, onInterruption: () => () => {} })
  stops.push(t.dispose); const listener = vi.fn(); t.subscribe(listener)
  ms = 2000; vi.advanceTimersByTime(500)
  expect(listener).toHaveBeenCalledOnce(); expect(t.capture()).toBeNull(); expect(vi.getTimerCount()).toBe(0)
})
it('cannot return a captured stamp after reentrant environment withdrawal', () => {
  let invalidate = () => {}, reenter = false
  const t = createAuthorityTimeline({ observe: () => { if (reenter) invalidate(); return { nowMs: 10, origin: 100 } },
    maxObservationGapMs: 1000, onInterruption(fn) { invalidate = fn; return () => {} } })
  stops.push(t.dispose); reenter = true; expect(t.capture()).toBeNull(); expect(vi.getTimerCount()).toBe(0)
})
it('releases a synchronously withdrawn interruption subscription exactly once', () => {
  const stop = vi.fn(), t = createAuthorityTimeline({ observe: () => ({ nowMs: 10, origin: 100 }), maxObservationGapMs: 1000,
    onInterruption(fn) { fn(); return stop } })
  expect(stop).toHaveBeenCalledOnce(); expect(t.capture()).toBeNull(); t.dispose(); expect(stop).toHaveBeenCalledOnce()
})
it('keeps a separate subscription when an identical listener unsubscribes once', () => {
  const t = createAuthorityTimeline({ observe: () => ({ nowMs: 10, origin: 100 }), maxObservationGapMs: 1000, onInterruption: () => () => {} })
  stops.push(t.dispose); const listener = vi.fn(), stop = t.subscribe(listener)
  t.subscribe(listener); stop(); t.dispose(); expect(listener).toHaveBeenCalledOnce()
})
it('has no browser performance authority in a non-browser environment', () => {
  const t = createBrowserAuthorityTimeline(1000, new AbortController().signal); stops.push(t.dispose)
  expect(t.capture()).toBeNull(); expect(vi.getTimerCount()).toBe(0)
})
it.each(['hidden', 'pagehide', 'bfcache', 'freeze', 'resume', 'request-abort'])('withdraws the actual browser adapter on %s lifecycle discontinuity', kind => {
  const document = Object.assign(new EventTarget(), { visibilityState: 'visible' })
  const window = Object.assign(new EventTarget(), { performance: { now: () => 100, timeOrigin: 1000 } })
  vi.stubGlobal('window', window); vi.stubGlobal('document', document)
  const lifetime = new AbortController(), t = createBrowserAuthorityTimeline(1000, lifetime.signal)
  stops.push(t.dispose); const listener = vi.fn(); t.subscribe(listener)
  window.dispatchEvent(Object.assign(new Event('pageshow'), { persisted: false }))
  expect(t.capture()).not.toBeNull()
  if (kind === 'hidden') { document.visibilityState = 'hidden'; document.dispatchEvent(new Event('visibilitychange')) }
  else if (kind === 'request-abort') lifetime.abort()
  else if (kind === 'freeze' || kind === 'resume') document.dispatchEvent(new Event(kind))
  else window.dispatchEvent(Object.assign(new Event(kind === 'bfcache' ? 'pageshow' : 'pagehide'), { persisted: true }))
  expect(listener).toHaveBeenCalledOnce(); expect(t.capture()).toBeNull(); expect(vi.getTimerCount()).toBe(0)
})
