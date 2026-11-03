// @vitest-environment jsdom
import { webcrypto } from 'node:crypto'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { attachMovement } from '../input/controller'
import { loadMap } from '../map/resources'
import { movementData } from './fixtures/movement-data'
import type { MovementIntent } from '../input/model'
const stops: (() => void)[] = []
let frames: Map<number, FrameRequestCallback>, serial = 0
beforeEach(() => {
  frames = new Map(); vi.stubGlobal('crypto', webcrypto)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { frames.set(++serial, callback); return serial })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id))
})
afterEach(() => { stops.splice(0).forEach(stop => stop()); document.body.replaceChildren(); vi.unstubAllGlobals() })
async function fixture(patch: Partial<Parameters<typeof attachMovement>[1]> = {}) {
  const data = await movementData(), map = await loadMap({ mapText: JSON.stringify(data.map), themeText: JSON.stringify(data.theme), pin: data.pin,
    resources: { read: async r => r.kind === 'png' ? data.png : data.tiledBytes }, decode: async () => ({ image: { naturalWidth: 96, naturalHeight: 32 } as HTMLImageElement, dispose() {} }) }, new AbortController().signal)
  const surface = document.createElement('canvas'), controls = document.createElement('div'), abort = new AbortController(), intents: MovementIntent[] = []
  document.body.append(surface, controls)
  const attachment = attachMovement({ host: surface, map, signal: abort.signal, camera: { surface: () => surface, center() {}, project: p => p, unproject: p => p,
    read: () => ({ zoom: 1, scrollX: 0, scrollY: 0, width: 390, height: 260 }) } }, { controls, zoom: 1, policy: { speedPxPerSecond: 120, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: 42 }, consume: i => intents.push(i), ...patch })
  stops.push(() => { attachment.dispose(); abort.abort(); map.dispose() }); attachment.setEnabled(true)
  const key = (code: string) => surface.dispatchEvent(new KeyboardEvent('keydown', { code, bubbles: true, cancelable: true }))
  const tick = () => { for (const time of [0, 50, 100]) { const callbacks = [...frames.values()]; frames.clear(); callbacks.forEach(fn => fn(time)) } }
  return { controls, surface, attachment, intents, key, tick }
}
it('keeps primary actions available and secondary DOM controls in an initially closed native disclosure', async () => {
  const f = await fixture(), details = f.controls.querySelector('details')!
  expect(details.open).toBe(false); expect(details.querySelector('summary')?.textContent).toBe('Controls & help')
  expect([...details.querySelectorAll('button')].map(b => b.getAttribute('aria-label'))).toEqual(['Zoom out', 'Zoom in'])
  expect(details.querySelector('select')?.getAttribute('aria-label')).toBe('Nearby interaction')
  expect(f.controls.querySelector('.world-controls-primary')?.textContent).toContain('Interact')
  expect(f.controls.querySelector('.world-controls-primary')?.textContent).toContain('Recenter')
  expect(details.textContent).toContain('arrows / W A S D'); expect(details.textContent).not.toContain('real arrival')
  f.attachment.setEnabled(false); details.open = true
  expect(f.controls.querySelector('summary')?.hasAttribute('disabled')).toBe(false)
  expect([...f.controls.querySelectorAll('button,select')].every(e => (e as HTMLButtonElement).disabled)).toBe(true)
})
it('moving focus into help neutralizes held movement without moving again from secondary-control keys', async () => {
  const f = await fixture(); f.surface.focus(); f.key('ArrowRight'); f.tick()
  expect(f.intents.some(i => i.axisX > 0)).toBe(true)
  const summary = f.controls.querySelector('summary')!; summary.focus()
  expect(f.intents.at(-1)).toEqual({ axisX: 0, axisY: 0, buttons: [] }); expect(frames.size).toBe(0)
  const foot = f.attachment.read().foot
  summary.dispatchEvent(new KeyboardEvent('keydown', { code: 'ArrowRight', bubbles: true })); f.tick()
  expect(f.attachment.read().foot).toEqual(foot)
})
it.each(['input', 'prediction'] as const)('keeps %s consumer errors outside the closed disclosure and visibly prominent', async kind => {
  const f = await fixture(kind === 'prediction' ? { onPrediction() { throw new Error('fixture') } } : { consume() { throw new Error('fixture') } })
  if (kind === 'input') { f.surface.focus(); f.key('ArrowRight'); f.tick() }
  const status = f.controls.querySelector('[role="status"]')!
  expect(status.textContent).toBe(`Movement unavailable: ${kind} consumer failed.`)
  expect(status.getAttribute('data-prominent')).toBe('true'); expect(status.closest('details')).toBeNull()
  f.attachment.setEnabled(false); expect(status.getAttribute('data-prominent')).toBe('true')
})
it('retains polite routine status and removes the complete disclosure on disposal', async () => {
  const f = await fixture(), status = f.controls.querySelector('[role="status"]')!
  expect(status.textContent).toBe('Local movement ready.'); expect(status.getAttribute('aria-live')).toBe('polite'); expect(status.getAttribute('data-prominent')).toBe('false')
  f.attachment.dispose(); expect(f.controls.children).toHaveLength(0)
})
