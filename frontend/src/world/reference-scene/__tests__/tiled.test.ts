import { expect, it } from 'vitest'
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import source from '../../../../public/world/reference-scene-v2/geometry-proposal.json'
import descriptor from '../../../../public/world/reference-scene-v2/runtime/source-binding-v2.json'
import { validateTiled } from '../../map/tiled'
import { referenceFixture } from './fixture'

it('binds the actual public finite Tiled bytes to the exact geometry and inert compatibility image', async () => {
  const bytes = await readFile(new URL('../../../../public/world/reference-scene-v2/runtime/canonical-map-v2.json', import.meta.url))
  expect('sha256:' + createHash('sha256').update(bytes).digest('hex')).toBe(descriptor.tilemap.sha256)
  expect(bytes.length).toBe(descriptor.tilemap.bytes)
  const texture = { image_state: 'PROVIDED', path: 'world/reference-scene-v2/runtime-v4/parade-ground.png', sha256: 'sha256:' + '0'.repeat(64), width_px: 768, height_px: 1296 }
  expect(() => validateTiled(bytes.toString(), source.geometry, texture)).not.toThrow()
  const data = JSON.parse(bytes.toString())
  expect(data.layers.map((l: { data: number[] }) => l.data)).toEqual([source.geometry.terrain_layer.flat(), source.geometry.collision_grid.flat(), source.geometry.overdraw_layer.flat()])
  expect(data.tilesets[0].image).toBe('mcr-theme.png'); expect(descriptor.status).toBe('NOT_ADMITTED')
  const fixture = await referenceFixture(); expect(new Uint8Array(bytes)).toEqual(fixture.tiled); expect(fixture.map.tilemap.path).toBe(descriptor.tilemap.path)
})
