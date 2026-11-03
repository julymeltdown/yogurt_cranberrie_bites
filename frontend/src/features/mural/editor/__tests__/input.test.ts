import { describe, expect, it, vi } from 'vitest'
import { boundView, createPointerInput, fitView, pixelAt, zoomView } from '../input'
function input() { const select = vi.fn(), pan = vi.fn(), cancel = vi.fn(); let clock = 0
  return { select, pan, cancel, advance: () => { clock += 700 }, input: createPointerInput({ select, pan, cancel, now: () => clock }) } }
describe('FE32 pointer disambiguation and bounded view', () => {
  it('tap previews once; automatic release of capture does not cancel the new preview', () => {
    const f = input(); f.input.down(1, { x: 10, y: 10 }); f.input.up(1, { x: 10, y: 10 }); f.input.lost(1)
    expect(f.select).toHaveBeenCalledExactlyOnceWith({ x: 10, y: 10 }); expect(f.cancel).toHaveBeenCalledTimes(1)
  })
  it('drag beyond threshold and back never selects', () => {
    const f = input(); f.input.down(1, { x: 10, y: 10 }); f.input.move(1, { x: 40, y: 10 }); f.input.move(1, { x: 10, y: 10 }); f.input.up(1, { x: 10, y: 10 })
    expect(f.pan).toHaveBeenCalledTimes(2); expect(f.select).not.toHaveBeenCalled()
  })
  it.each(['cancel', 'lost', 'longpress', 'multitouch'] as const)('%s cannot select', kind => {
    const f = input(); f.input.down(1, { x: 10, y: 10 })
    if (kind === 'cancel') f.input.cancel()
    if (kind === 'lost') f.input.lost(1)
    if (kind === 'longpress') f.advance()
    if (kind === 'multitouch') { f.input.down(2, { x: 20, y: 20 }); f.input.up(2, { x: 20, y: 20 }) }
    f.input.up(1, { x: 10, y: 10 }); expect(f.select).not.toHaveBeenCalled()
  })
  it('bounds pan, zoom, and transformed hit testing', () => {
    expect(boundView({ zoom: 50, x: 100, y: -9000 }, 300)).toEqual({ zoom: 8, x: 0, y: -2100 })
    const zoom = zoomView(fitView(), 4, 300); expect(zoom).toEqual({ zoom: 4, x: -450, y: -450 })
    expect(pixelAt({ x: 150, y: 150 }, zoom, 300)).toEqual({ x: 64, y: 64 })
    expect(pixelAt({ x: 300, y: 0 }, fitView(), 300)).toBeNull(); expect(pixelAt({ x: -1, y: 0 }, fitView(), 300)).toBeNull()
    expect(pixelAt({ x: 0, y: 0 }, fitView(), 0)).toBeNull()
  })
})
