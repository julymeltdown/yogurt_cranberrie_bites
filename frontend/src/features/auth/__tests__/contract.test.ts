import { describe, expect, it, vi } from 'vitest'
import { canonicalJson, decodeBootstrap } from '../model/bootstrap'
import { decodeAuth, decodeCapabilities, decodeContract, validLogin } from '../model/contract'
import { authResponse, capabilities, eventId, fixture, userId } from './fixtures/data'
import { validBootstrap } from '../../../app/access'
const scope = { generation: 1, userId, eventId }
const body = () => fixture() as unknown as Record<string, any> // test mutation harness only
async function rehash(document: Record<string, any>) {
  const { content_hash: _, ...value } = document
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(canonicalJson(value)))
  document.content_hash = `sha256:${Array.from(new Uint8Array(digest), x => x.toString(16).padStart(2, '0')).join('')}`
}
describe('canonical C03 auth boundaries', () => {
  it('accepts the published examples and keeps only memory credentials/lifetime', () => {
    expect(decodeAuth(authResponse())).toEqual({ userId, accessToken: 'fixture-not-a-real-credential', expiresIn: 900 })
    expect(decodeCapabilities(capabilities())).toEqual({ email: true })
  })
  it.each([
    { refreshToken: 'secret' }, { tokenType: 'Basic' }, { expiresIn: 1.5 }, { expiresIn: 0 }, { expiresIn: Number.MAX_SAFE_INTEGER + 1 },
    { accessToken: '' }, { accessToken: 'unsafe\nheader' }, { userId: userId.toUpperCase().replace('101', '10A') },
    { extra: 'unrecognized' }, { linkedProviders: ['GOOGLE', 'GOOGLE'] }, { linkedProviders: ['DISCORD'] },
  ])('rejects invalid auth response %j', patch => expect(() => decodeAuth({ ...authResponse(), ...patch })).toThrow())
  it('rejects missing auth fields and contradictory/incomplete/duplicate provider capabilities', () => {
    const missing = authResponse() as Partial<ReturnType<typeof authResponse>>; delete missing.refreshToken
    expect(() => decodeAuth(missing)).toThrow()
    const c = capabilities(); c.providers[0]!.disabled_reason = 'NOT_CONFIGURED'
    expect(() => decodeCapabilities(c)).toThrow()
    expect(() => decodeCapabilities({ ...capabilities(), providers: capabilities().providers.slice(0, 3) })).toThrow()
    expect(() => decodeCapabilities({ ...capabilities(), providers: Array(4).fill(capabilities().providers[0]) })).toThrow()
  })
  it('measures login passwords in UTF-8 without trimming or imposing signup policy', () => {
    expect(validLogin('fan@example.com', ' ')).toBe(true)
    expect(validLogin('fan@example.com', 'é'.repeat(64))).toBe(true)
    expect(validLogin('fan@example.com', 'é'.repeat(65))).toBe(false)
    expect(validLogin('fan@example.com', '😀'.repeat(32))).toBe(true)
    expect(validLogin('fan@example.com', '😀'.repeat(33))).toBe(false)
    expect(validLogin('fan@example.com', '\ud800')).toBe(false)
    expect(validLogin('fan@example.com', '')).toBe(false)
    expect(validLogin('broken', 'password')).toBe(false)
  })
})
describe('full bootstrap and C06 before shell projection', () => {
  it('preserves and freezes all seven scoped capability decisions and their reasons', async () => {
    const b = body()
    for (const name of Object.keys(b.capabilities)) b.capabilities[name] = { available: true, reason: null }
    const enabled = await decodeBootstrap(b, scope)
    for (const name of Object.keys(b.capabilities)) b.capabilities[name] = { available: false, reason: 'FEATURE_DISABLED' }
    const disabled = await decodeBootstrap(b, scope)
    expect(enabled.capabilities).not.toEqual(disabled.capabilities)
    expect(Object.keys(disabled.capabilities)).toHaveLength(7)
    expect(Object.isFrozen(disabled.capabilities)).toBe(true)
    expect(Object.values(disabled.capabilities).every(Object.isFrozen)).toBe(true)
    expect(disabled.capabilities.meet).toEqual({ available: false, reason: 'FEATURE_DISABLED' })
    expect(disabled.capabilities.gifts).toEqual({ available: false, reason: 'FEATURE_DISABLED' })
    expect(validBootstrap(disabled, scope)).toBe(true)
    expect(validBootstrap({ ...disabled, worldAvailable: true }, scope)).toBe(false)
    expect(validBootstrap({ ...enabled, event: { ...enabled.event, phase: 'READ_ONLY' } }, scope)).toBe(false)
    const incomplete = { ...disabled, capabilities: { ...disabled.capabilities } } as any
    delete incomplete.capabilities.meet
    expect(validBootstrap(incomplete, scope)).toBe(false)
    b.capabilities.meet.reason = 'DEPENDENCY_UNAVAILABLE'
    expect(disabled.capabilities.meet.reason).toBe('FEATURE_DISABLED')
  })
  it('accepts published fields and retains the validated resource document for the shell lease', async () => {
    expect(await decodeBootstrap(fixture(), scope)).toEqual({ contractVersion: '0.1.0',
      event: { eventId, title: 'Synthetic contract fixture', phase: 'OPEN' }, participant: { userId, eventId }, worldAvailable: false,
      capabilities: fixture().capabilities, document: fixture() })
  })
  it('owns asset bytes before asynchronous validation and prevents later release mutation', async () => {
    const input = body(), original = structuredClone(input)
    const pending = decodeBootstrap(input, scope)
    input.map_manifest.geometry.default_spawn_id = 'unreviewed_spawn'
    input.avatar.config.item_ids.hair = 'unreviewed_hair'
    const result = await pending
    expect(result.document).toEqual(original)
    const document = result.document as ReturnType<typeof body>
    expect(() => { document.map_manifest.geometry.default_spawn_id = 'replacement' }).toThrow()
    expect(() => { document.theme_manifests.push({}) }).toThrow()
    expect(result.document).toEqual(original)
  })
  it.each([
    ['additional private field', (b: ReturnType<typeof body>) => { b.participant.profile.email = 'private@example.com' }],
    ['missing required nested field', (b: ReturnType<typeof body>) => { delete b.participant.preferences.sound_enabled }],
    ['wrong user', (b: ReturnType<typeof body>) => { b.participant.user_id = '00000000-0000-4000-8000-000000000999' }],
    ['wrong event', (b: ReturnType<typeof body>) => { b.event.event_id = '00000000-0000-4000-8000-000000000999' }],
    ['wrong profile identity', (b: ReturnType<typeof body>) => { b.participant.profile.user_id = '00000000-0000-4000-8000-000000000999' }],
    ['avatar divergence', (b: ReturnType<typeof body>) => { b.avatar.revision++ }],
    ['READ_ONLY writes', (b: ReturnType<typeof body>) => { b.event.phase = 'READ_ONLY'; b.capabilities.mutations = { available: true, reason: null } }],
    ['ARCHIVED writes', (b: ReturnType<typeof body>) => { b.event.phase = 'ARCHIVED'; b.capabilities.mutations = { available: true, reason: null } }],
    ['contradictory capability', (b: ReturnType<typeof body>) => { b.capabilities.world.reason = 'NOT_AVAILABLE' }],
    ['missing theme', (b: ReturnType<typeof body>) => { b.theme_manifests.pop() }],
    ['duplicate themes', (b: ReturnType<typeof body>) => { b.theme_manifests[1] = b.theme_manifests[0] }],
    ['unknown layer item', (b: ReturnType<typeof body>) => { b.avatar.config.item_ids.hair = b.participant.profile.avatar.config.item_ids.hair = 'unknown-hair' }],
    ['forged content hash', (b: ReturnType<typeof body>) => { b.map_manifest.content_hash = 'sha256:' + '0'.repeat(64) }],
    ['bad resource state', (b: ReturnType<typeof body>) => { b.map_manifest.tilemap.resource_state = 'PROVIDED' }],
    ['invalid UTC date', (b: ReturnType<typeof body>) => { b.server_time = '2026-02-30T00:00:00Z' }],
    ['offset server time', (b: ReturnType<typeof body>) => { b.server_time = '2026-11-03T09:00:00+09:00' }],
  ])('rejects %s', async (_name, change) => { const b = body(); change(b); await expect(decodeBootstrap(b, scope)).rejects.toThrow() })
  it.each([
    ['theme map binding', async (b: ReturnType<typeof body>) => { b.theme_manifests[0].map_revision++; await rehash(b.theme_manifests[0]) }],
    ['duplicate atlas ID', async (b: ReturnType<typeof body>) => { b.avatar_manifest.atlas.items[1].item_id = b.avatar_manifest.atlas.items[0].item_id; await rehash(b.avatar_manifest) }],
    ['overlapping atlas cells', async (b: ReturnType<typeof body>) => { b.avatar_manifest.atlas.items[1].region = b.avatar_manifest.atlas.items[0].region; await rehash(b.avatar_manifest) }],
    ['unknown fallback preset', async (b: ReturnType<typeof body>) => { b.avatar_manifest.fallback_preset_id = 'unknown-preset'; await rehash(b.avatar_manifest) }],
    ['wrong theme places', async (b: ReturnType<typeof body>) => { b.theme_manifests[0].place_ids[0] = 'unknown-place'; await rehash(b.theme_manifests[0]) }],
    ['misaligned theme texture', async (b: ReturnType<typeof body>) => { b.theme_manifests[0].texture.width_px++; await rehash(b.theme_manifests[0]) }],
    ['duplicate theme places', async (b: ReturnType<typeof body>) => { b.theme_manifests[0].place_ids.push(b.theme_manifests[0].place_ids[0]); await rehash(b.theme_manifests[0]) }],
  ])('rejects independently rehashed %s', async (_name, change) => { const b = body(); await change(b); await expect(decodeBootstrap(b, scope)).rejects.toThrow() })
  it('rejects non-safe integers even when nested schema has no maximum', () => {
    const b = body(); b.map_manifest.geometry.width_cells = 1e100
    expect(() => decodeContract('Bootstrap', b)).toThrow()
  })
})

async function seal(b: ReturnType<typeof body>) {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(canonicalJson(b.map_manifest.geometry)))
  b.map_manifest.geometry_hash = `sha256:${Array.from(new Uint8Array(digest), x => x.toString(16).padStart(2, '0')).join('')}`
  await rehash(b.map_manifest); await rehash(b.avatar_manifest)
  for (const theme of b.theme_manifests) {
    theme.map_content_hash = b.map_manifest.content_hash; theme.geometry_hash = b.map_manifest.geometry_hash
    theme.avatar_content_hash = b.avatar_manifest.content_hash
    await rehash(theme)
  }
}
it.each([
  ['unreachable interaction', (b: ReturnType<typeof body>) => { b.map_manifest.geometry.collision_grid[1][2] = 1; b.map_manifest.geometry.collision_grid[2][1] = 1 }],
  ['grid dimensions', (b: ReturnType<typeof body>) => { b.map_manifest.geometry.width_cells++ }],
  ['blocked spawn', (b: ReturnType<typeof body>) => { b.map_manifest.geometry.collision_grid[1][1] = 1 }],
  ['unknown interaction place', (b: ReturnType<typeof body>) => { b.map_manifest.geometry.interactions[0].place_id = '00000000-0000-4000-8000-000000000999' }],
  ['fixture venue claimed as real', (b: ReturnType<typeof body>) => { b.map_manifest.places[0].verification_state = 'REVIEWED'; b.map_manifest.places[0].place_kind = 'REAL_MEETING' }],
  ['overlapping atlas after rebinding every digest', (b: ReturnType<typeof body>) => { b.avatar_manifest.atlas.items[1].region = b.avatar_manifest.atlas.items[0].region }],
])('rejects %s with internally valid recomputed hashes', async (_label, change) => {
  const b = body(); change(b); await seal(b)
  await expect(decodeBootstrap(b, scope)).rejects.toThrow()
})

it('fails closed when a future canonical schema introduces an unsupported assertion', async () => {
  vi.resetModules()
  vi.doMock('../../../../../contracts/generated/mcr-core.openapi.json', async importOriginal => {
    const original = await importOriginal<{ default: { components: { schemas: Record<string, Record<string, unknown>> } } }>()
    const copy = structuredClone(original.default)
    copy.components.schemas.Bootstrap!.unevaluatedProperties = false
    return { default: copy }
  })
  try {
    const changed = await import('../model/contract')
    expect(() => changed.decodeContract('Bootstrap', fixture())).toThrow()
  } finally { vi.doUnmock('../../../../../contracts/generated/mcr-core.openapi.json'); vi.resetModules() }
})
