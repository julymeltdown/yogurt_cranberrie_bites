// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createBrowserLocation } from '../../../../app/venue-entry/browser-location'

const releases: (() => void)[] = []
afterEach(() => { releases.splice(0).forEach(stop => stop()); Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' }) })
function setup() {
  const abort = new AbortController(), clearWatch = vi.fn(), fixes = vi.fn(), withdrawal = vi.fn()
  let success!: PositionCallback, failure!: PositionErrorCallback
  const watchPosition = vi.fn((ok: PositionCallback, error?: PositionErrorCallback | null) => { success = ok; failure = error!; return 7 })
  const phone = createBrowserLocation({ signal: abort.signal, geolocation: { watchPosition, clearWatch } }); releases.push(phone.dispose)
  const emit = (latitude = 1) => success({ coords: { latitude, longitude: 2, accuracy: 5, altitude: null, altitudeAccuracy: null, heading: null, speed: null, toJSON() { return {} } }, timestamp: 123, toJSON() { return {} } })
  return { phone, abort, watchPosition, clearWatch, fixes, withdrawal, emit, fail: () => failure({ code: 1, message: 'denied', PERMISSION_DENIED: 1, POSITION_UNAVAILABLE: 2, TIMEOUT: 3 }) }
}
it('does not query or start browser geolocation on construction or subscription', () => {
  const h = setup(); h.phone.subscribe(() => {}); expect(h.watchPosition).not.toHaveBeenCalled()
})
it('starts only on an explicit call and ignores late positions after stop', () => {
  const h = setup(); h.phone.start(h.fixes, h.withdrawal); h.emit(); expect(h.fixes).toHaveBeenCalledOnce(); h.phone.stop(); h.emit(); expect(h.fixes).toHaveBeenCalledOnce(); expect(h.clearWatch).toHaveBeenCalledWith(7)
})
it('stops on background without automatically resuming when visible', () => {
  const h = setup(); h.phone.start(h.fixes, h.withdrawal)
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' }); document.dispatchEvent(new Event('visibilitychange')); h.emit()
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' }); document.dispatchEvent(new Event('visibilitychange'))
  expect(h.withdrawal).toHaveBeenCalledWith('background'); expect(h.fixes).not.toHaveBeenCalled(); expect(h.watchPosition).toHaveBeenCalledOnce()
})
it('clears the watch and rejects late callbacks on canonical abort', () => {
  const h = setup(); h.phone.start(h.fixes, h.withdrawal); h.abort.abort(); h.emit(); expect(h.clearWatch).toHaveBeenCalledWith(7); expect(h.fixes).not.toHaveBeenCalled()
})
it('exposes denied state and clears its watch', () => {
  const h = setup(); h.phone.start(h.fixes, h.withdrawal); h.fail(); expect(h.phone.read()).toBe('denied'); expect(h.withdrawal).toHaveBeenCalledWith('denied'); expect(h.clearWatch).toHaveBeenCalledWith(7)
})
it('rejects invalid coordinates before any capability receives a fix', () => {
  const h = setup(); h.phone.start(h.fixes, h.withdrawal); h.emit(100); expect(h.fixes).not.toHaveBeenCalled(); expect(h.withdrawal).toHaveBeenCalledWith('unavailable')
})
it('fails closed when the browser API is absent', () => {
  const abort = new AbortController(), phone = createBrowserLocation({ signal: abort.signal, geolocation: null }), missing = vi.fn(); releases.push(phone.dispose)
  phone.start(() => { throw Error('Must not emit') }, missing); expect(phone.read()).toBe('unavailable'); expect(missing).toHaveBeenCalledWith('unavailable')
})
