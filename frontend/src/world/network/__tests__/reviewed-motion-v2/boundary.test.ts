import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { expect, expectTypeOf, it, vi } from 'vitest'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import layered from '../../../../../../contracts/fixtures/assets/avatar-config.json'
import whole from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { decode, WorldTransportError } from '../../transport/contract'
import type { AvatarConfig, AvatarState, ReviewedMotionAvatarConfig, WorldPlayer } from '../../transport/wire-types'

const valid = import.meta.glob('../../../../../../contracts/fixtures/assets/motion-v2/reviewed/valid/*.json', { eager: true, import: 'default' })
const invalid = import.meta.glob('../../../../../../contracts/fixtures/assets/motion-v2/reviewed/invalid/*.json', { eager: true, import: 'default' })
const candidate = import.meta.glob('../../../../../../contracts/fixtures/assets/motion-v2/valid/*.json', { eager: true, import: 'default' })
const snapshot = movement.messages.find(row => row.message.type === 'snapshot')!.message
const player = snapshot.players![0]!
function frame(config: unknown) {
  return { ...snapshot, players: [{ ...player, avatar: { revision: 1, config } }] }
}

it('uses the complete canonical AvatarState union while retaining the legacy AvatarConfig definition', () => {
  expectTypeOf<AvatarState['config']>().toEqualTypeOf<AvatarConfig | ReviewedMotionAvatarConfig>()
  expectTypeOf<WorldPlayer['avatar']['config']>().toEqualTypeOf<AvatarState['config']>()
  expectTypeOf<Extract<AvatarState['config'], { contract_version: '2.0.0' }>>().toEqualTypeOf<ReviewedMotionAvatarConfig>()
  expectTypeOf<Extract<AvatarConfig, { contract_version: '2.0.0' }>>().toEqualTypeOf<never>()
})

it.each([
  ['avatar-config', 'ReviewedMotionAvatarConfig', '8d4b5772c763a32e58dd49bf5115894703bba603c6e2c01333cef5a86c988c2e'],
  ['avatar-manifest', 'ReviewedMotionAvatarManifest', '3fd9bfd589a7f4d2749081ef76057f8bbaf488fbf7385c4cc024ffd01790ccbf'],
])('pins the exact producer %s bytes and unique public definition', (name, definition, hash) => {
  const bytes = readFileSync(new URL(`../../../../../../contracts/assets/v2/reviewed/${name}.schema.json`, import.meta.url))
  const schema = JSON.parse(bytes.toString())
  expect(createHash('sha256').update(bytes).digest('hex')).toBe(hash)
  expect(schema.$id).toBe(`https://contracts.mcr.invalid/assets/v2/reviewed/${name}.schema.json`)
  expect(schema.$ref).toBe(`#/$defs/${definition}`)
  expect(Object.keys(schema.$defs)).toEqual([definition])
})

it('discovers the complete pinned fixture matrix', () => {
  expect(Object.keys(valid)).toHaveLength(15)
  expect(Object.keys(invalid)).toHaveLength(16)
  expect(Object.keys(candidate)).toHaveLength(15)
})

it.each([...Object.entries(valid), ['legacy 1.0', layered], ['legacy 1.1', whole.presets[0]!.config]])('retains exact valid config %s in the actual C05 decoder', (_path, config) => {
  const value = frame(config)
  expect(decode(JSON.stringify(value), 'server')).toEqual(value)
})
it.each([...Object.entries(invalid), ...Object.entries(candidate)])('rejects malformed or unreviewed candidate config %s', (_path, config) => {
  expect(() => decode(JSON.stringify(frame(config)), 'server')).toThrow(WorldTransportError)
})
it.each([
  { ...Object.values(valid)[0] as object, email: 'private@example.invalid' },
  { ...Object.values(valid)[0] as object, skin_tone_id: 'independent_skin_forbidden' },
  { ...whole.presets[0]!.config, representation: 'motion_preset' },
])('rejects private, mixed and unsupported config fields %#', config => {
  expect(() => decode(JSON.stringify(frame(config)), 'server')).toThrow(WorldTransportError)
})

it('fails closed when a canonical config reference names an unregistered document', async () => {
  vi.resetModules()
  vi.doMock('../../../../../../contracts/core/v1/core.schema.json', async importOriginal => {
    const original = await importOriginal<{ default: { $defs: Record<string, unknown> } }>()
    const copy = structuredClone(original.default)
    copy.$defs.AvatarState = { type: 'object', properties: { config: { $ref: 'https://contracts.mcr.invalid/unregistered.json#/$defs/ReviewedMotionAvatarConfig' } } }
    return { default: copy }
  })
  try {
    const changed = await import('../../transport/contract')
    expect(() => changed.decode(JSON.stringify(frame(Object.values(valid)[0])), 'server')).toThrow()
  } finally {
    vi.doUnmock('../../../../../../contracts/core/v1/core.schema.json')
    vi.resetModules()
  }
})
