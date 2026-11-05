import { describe, expect, it } from 'vitest'
import { createCameraControl, type WorldCamera } from '../../../../../world/camera/port'
describe('logical zoom with fitted display', () => {
  it('steps through the full logical range while preserving actual renderer scale and native foot', () => {
    let logical = 1, foot = { x: 472, y: 424 }
    const calls: { foot: { x: number; y: number }; zoom: number }[] = []
    const view: WorldCamera = {
      surface: () => null,
      center(point, zoom) { foot = point; logical = zoom; calls.push({ foot: { ...point }, zoom }) },
      project: p => p, unproject: p => p,
      read: () => ({ zoom: logical * .6541095890410958, scrollX: 261, scrollY: 132, width: 382, height: 502 }),
    }
    const control = createCameraControl(view, foot, 1)
    expect(control.read().camera!.zoom).toBe(1)
    expect(view.read()!.zoom).toBeCloseTo(.6541095890410958)
    for (let target = 2; target <= 8; target++) {
      expect(control.setZoom(Math.min(8, control.read().camera!.zoom + 1))).toBe(true)
      expect(control.read().camera!.zoom).toBe(target)
      expect(view.read()!.zoom).toBeCloseTo(target * .6541095890410958)
    }
    for (let target = 7; target >= 1; target--) {
      control.setZoom(Math.max(1, control.read().camera!.zoom - 1))
      expect(control.read().camera!.zoom).toBe(target)
    }
    expect(calls.every(call => call.foot.x === 472 && call.foot.y === 424)).toBe(true)
    control.setZoom(3); control.lookAt({ x: 504, y: 760 }); control.recenter()
    expect(control.read()).toMatchObject({ following: true, camera: { zoom: 3 } })
    expect(foot).toEqual({ x: 472, y: 424 })
    expect(control.setZoom(9)).toBe(false)
    expect(control.setZoom(0)).toBe(false)
  })
  it('preserves nullable snapshots and reads the actual view exactly once', () => {
    let reads = 0
    const view: WorldCamera = { surface: () => null, center() {}, project: p => p, unproject: p => p,
      read: () => { reads++; return null } }
    const control = createCameraControl(view, { x: 472, y: 424 }, 1)
    expect(control.read()).toEqual({ following: true, camera: null })
    expect(reads).toBe(1)
  })
})
