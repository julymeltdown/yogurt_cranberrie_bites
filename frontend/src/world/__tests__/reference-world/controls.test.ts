// @vitest-environment jsdom
import { webcrypto } from 'node:crypto'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { attachMovement } from '../../input/controller'
import { movementData } from '../fixtures/movement-data'
import { loadMap } from '../../map/resources'
const cleanup: (() => void)[] = []
beforeEach(() => { vi.stubGlobal('crypto', webcrypto); vi.stubGlobal('requestAnimationFrame', () => 1); vi.stubGlobal('cancelAnimationFrame', () => {}) })
afterEach(() => { for (const stop of cleanup.splice(0)) stop(); document.body.replaceChildren(); vi.unstubAllGlobals() })
async function fixture(preexisting = false, radius = 42) {
  const data = await movementData(), map = await loadMap({ mapText: JSON.stringify(data.map), themeText: JSON.stringify(data.theme), pin: data.pin,
    resources: { read: async r => r.kind === 'png' ? data.png : data.tiledBytes }, decode: async () => ({ image: { naturalWidth: 96, naturalHeight: 32 } as HTMLImageElement, dispose() {} }) }, new AbortController().signal)
  const host = document.createElement('div'), controls = document.createElement('div'), surface = document.createElement('canvas'), abort = new AbortController()
  host.append(surface); document.body.append(host, controls)
  if (preexisting) { host.classList.add('world-controls-stage'); controls.classList.add('world-controls-host') }
  const attachment = attachMovement({ host, map, signal: abort.signal, camera: { surface: () => surface, center() {}, project: p => p, unproject: p => p,
    read: () => ({ zoom: 1, scrollX: 0, scrollY: 0, width: 390, height: 400 }) } }, { controls, zoom: 1,
    policy: { speedPxPerSecond: 96, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: radius }, consume() {} })
  cleanup.push(() => { attachment.dispose(); abort.abort(); map.dispose() }); attachment.setEnabled(true)
  return { host, controls, attachment }
}
it('Escape closes an expanded disclosure and returns keyboard focus without losing the help name', async () => {
  const f = await fixture(), details = f.controls.querySelector('details')!, summary = f.controls.querySelector('summary')!, select = f.controls.querySelector('select')!
  details.open = true; select.focus()
  const escape = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }); select.dispatchEvent(escape)
  expect(details.open).toBe(false); expect(escape.defaultPrevented).toBe(true); expect(document.activeElement).toBe(summary); expect(summary.textContent).toBe('Controls & help')
})
it.each([false, true])('restores root-owned host classes on teardown (preexisting=%s)', async preexisting => {
  const f = await fixture(preexisting)
  expect(f.host.classList.contains('world-controls-stage')).toBe(true); expect(f.controls.classList.contains('world-controls-host')).toBe(true)
  f.attachment.dispose()
  expect(f.host.classList.contains('world-controls-stage')).toBe(preexisting); expect(f.controls.classList.contains('world-controls-host')).toBe(preexisting); expect(f.controls.children).toHaveLength(0)
})

it('shows an explicit empty nearby state on the first refresh', async () => {
  const f = await fixture(false, 1), select = f.controls.querySelector('select')!
  expect(select.disabled).toBe(true); expect(select.options).toHaveLength(1); expect(select.options[0]?.textContent).toBe('No reachable interaction nearby')
})
