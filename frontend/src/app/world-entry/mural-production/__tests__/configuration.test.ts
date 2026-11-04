import { describe, expect, it, vi } from 'vitest'
import placement from '../placement.json'
import { readMuralProductionConfiguration, validateMuralProductionConfiguration } from '../configuration'

describe('explicit product-virtual mural placement', () => {
  it('detaches and freezes the qualified map and raster placement', () => {
    const input = structuredClone(placement), value = validateMuralProductionConfiguration(input)!
    input.anchor.x = 1; input.map.revision = 999
    expect(value.anchor).toEqual({ x: 312, y: 624, width: 128, height: 128, depth: 0.5 })
    expect(value.map.revision).toBe(4)
    expect(Object.isFrozen(value)).toBe(true)
    expect(Object.isFrozen(value.anchor)).toBe(true)
    expect(Object.isFrozen(value.map)).toBe(true)
    expect(readMuralProductionConfiguration(JSON.stringify(placement))).toEqual(value)
  })
  it.each([
    null, undefined, {}, { ...placement, kind: 'PHYSICAL_PLACE' }, { ...placement, permission: 'EDITABLE' },
    { ...placement, sourceRevision: 'unqualified' }, { ...placement, placementHash: 'unpinned' },
    { ...placement, textureKey: 'existing-art' }, { ...placement, anchor: { ...placement.anchor, x: -1 } },
    { ...placement, anchor: { ...placement.anchor, x: 760 } },
    { ...placement, anchor: { ...placement.anchor, width: 0 } },
    { ...placement, anchor: { ...placement.anchor, depth: Infinity } },
    { ...placement, map: { ...placement.map, widthPx: 20000 } },
    { ...placement, map: { ...placement.map, geometryHash: 'sha256:incorrect' } },
  ])('rejects malformed or unqualified input %j', input => {
    expect(validateMuralProductionConfiguration(input)).toBeNull()
  })
  it('rejects accessors and inherited or symbol-bearing inputs without evaluating them', () => {
    const getter = vi.fn(() => placement.anchor), input = { ...placement }
    Object.defineProperty(input, 'anchor', { enumerable: true, get: getter })
    expect(validateMuralProductionConfiguration(input)).toBeNull()
    expect(getter).not.toHaveBeenCalled()
    expect(validateMuralProductionConfiguration(Object.create(placement))).toBeNull()
    expect(validateMuralProductionConfiguration({ ...placement, [Symbol('permission')]: true })).toBeNull()
  })
  it.each(['', '{}', '{', 'x'.repeat(8193), 1])('fails closed for absent or invalid environment configuration', raw => {
    expect(readMuralProductionConfiguration(raw)).toBeNull()
  })
})
