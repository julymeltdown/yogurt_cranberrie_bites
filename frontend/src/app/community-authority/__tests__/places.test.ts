import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { checkedBootstrap, fixture, physicalBootstrap } from './fixtures'
const stops: (() => void)[] = []
beforeEach(() => vi.useFakeTimers())
afterEach(() => { stops.splice(0).reverse().forEach(fn => fn()); vi.useRealTimers() })
describe('public C03 place projection, never arrival authority', () => {
  it('preserves the exact virtual ID/kind/provenance but offers no physical choice', async () => {
    const bootstrap = await checkedBootstrap(), f = fixture(bootstrap); stops.push(f.dispose)
    const input = (bootstrap.document!.map_manifest as { places: unknown[] }).places[0]
    const value = f.authority.publicPlaces.read()!, p = value.items[0]!
    const { id, ...canonical } = p
    expect(canonical).toEqual(input); expect(id).toBe(p.place_id)
    expect(p.place_kind).toBe('VIRTUAL_CONCEPT'); expect(f.authority.places.read()!.items).toEqual([])
    expect(Object.keys(value).sort()).toEqual(['items', 'scope', 'source'])
    expect(Object.keys(p)).not.toContain('arrived'); expect(Object.keys(p)).not.toContain('revision')
    expect(Object.isFrozen(value.items)).toBe(true); expect(Object.isFrozen(p.provenance)).toBe(true)
  })
  it('offers only actual open reviewed physical facts without manufacturing audience or permission', async () => {
    const bootstrap = await physicalBootstrap(), f = fixture(bootstrap); stops.push(f.dispose)
    const all = f.authority.publicPlaces.read()!, eligible = f.authority.places.read()!
    expect(eligible.items).toEqual(all.items); expect(eligible.items).toHaveLength(1)
    const p = eligible.items[0]!
    expect(p.place_kind).toBe('PHYSICAL'); expect(p.provenance.source_uri).toBe('https://venue.example.invalid/source')
    expect(p).not.toHaveProperty('audience'); expect(p).not.toHaveProperty('expiresAt'); expect(p).not.toHaveProperty('canCreate')
    expect(eligible.source.mapRevision).toBe((bootstrap.document!.map_manifest as { map_revision: number }).map_revision)
  })
  it.each(['closed', 'unreviewed', 'unsafe-source'] as const)('retains canonical %s facts but disables the label-only physical choice', async kind => {
    const bootstrap = await physicalBootstrap(place => {
      if (kind === 'closed') place.closure_state = 'CLOSED'
      if (kind === 'unreviewed') {
        place.verification_state = 'UNVERIFIED'
        Object.assign(place.provenance as object, { review_state: 'UNREVIEWED', reviewed_by: null, reviewed_at: null })
      }
      if (kind === 'unsafe-source') Object.assign(place.provenance as object, { source_uri: 'javascript:alert(1)' })
    })
    const f = fixture(bootstrap); stops.push(f.dispose)
    expect(f.authority.publicPlaces.read()!.items).toHaveLength(1); expect(f.authority.places.read()!.items).toEqual([])
  })
  it('public choices withdraw with clock authority and cannot grant a beacon or roster', async () => {
    const f = fixture(await physicalBootstrap()); stops.push(f.dispose)
    expect(Object.keys(f.authority).sort()).toEqual(['clock', 'dispose', 'places', 'publicPlaces', 'readInterval'])
    f.interrupt(); expect(f.authority.publicPlaces.read()).toBeNull(); expect(f.authority.places.read()).toBeNull()
  })
})
