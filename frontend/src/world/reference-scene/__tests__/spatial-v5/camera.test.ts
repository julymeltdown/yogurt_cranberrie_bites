import { expect, it } from 'vitest'
import { spatialV5CameraPresentation } from '../../spatial-v5/camera'
import { spatialV5Fixture } from './fixture'

it('fits authored v10 framing and follows distant feet without mutating logical policy', async () => {
  const f = await spatialV5Fixture(), map = { map: f.map, theme: f.theme, widthPx: 768, heightPx: 1296 }
  const focus = Object.freeze({ x: 472, y: 440 })
  for (const width of [348, 378, 418]) {
    expect(spatialV5CameraPresentation(map, { width, height: 570 }, focus, 1)).toEqual({ zoom: width / 584, center: { x: 452, y: 305 } })
    expect(spatialV5CameraPresentation(map, { width, height: 570 }, focus, 2)!.zoom).toBe(2 * width / 584)
  }
  expect(focus).toEqual({ x: 472, y: 440 })
  expect(spatialV5CameraPresentation(map, { width: 1000, height: 292 }, focus, 1)!.zoom).toBe(.5)
  expect(spatialV5CameraPresentation(map, { width: 1000, height: 1000 }, focus, 1)!.zoom).toBe(1)
  expect(spatialV5CameraPresentation(map, { width: 378, height: 570 }, { x: 650, y: 900 }, 1)!.center).toEqual({ x: 618, y: 740 })
  for (const zoom of [0, .5, 9, NaN]) expect(spatialV5CameraPresentation(map, { width: 378, height: 570 }, focus, zoom)).toBeNull()
  expect(spatialV5CameraPresentation(map, { width: 0, height: 570 }, focus, 1)).toBeNull()
  expect(spatialV5CameraPresentation({ ...map, map: { ...f.map, geometry_hash: 'sha256:' + '0'.repeat(64) } }, { width: 378, height: 570 }, focus, 1)).toBeNull()
})
