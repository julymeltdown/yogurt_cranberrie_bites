import { expect, it } from 'vitest'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import layered from '../../../../../../contracts/fixtures/assets/avatar-config.json'
import wholeManifest from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { decode, WorldTransportError } from '../../../../world/network/transport/contract'
import { canonicalJson } from '../../../../world/map/json'
import type { Snapshot, Delta, WorldPlayer } from '../../../../world/network/transport/wire-types'

const snapshot = movement.messages.find(row => row.message.type === 'snapshot')!.message as unknown as Snapshot
const delta = movement.messages.find(row => row.message.type === 'delta')!.message as unknown as Delta
function frame(type: 'snapshot' | 'delta', configs: unknown[]) {
  const template = type === 'snapshot' ? snapshot : delta
  const player = snapshot.players[0]!
  const players = configs.map((config, i) => ({ ...player, userId: i ? '00000000-0000-4000-8000-000000000099' : player.userId, avatar: { revision: i + 1, config } }))
  return { ...template, ...(type === 'snapshot' ? { players } : { upserts: players, removes: [] }) }
}
for (const type of ['snapshot', 'delta'] as const) {
  it(`C05 ${type} retains both validated variants together without converting or defaulting`, () => {
    const expected = [layered, wholeManifest.presets[5]!.config]
    const input = frame(type, expected), decoded = decode(JSON.stringify(input), 'server')
    expect(decoded.type).toBe(type)
    const players: readonly WorldPlayer[] = decoded.type === 'snapshot' ? decoded.players : (decoded as Delta).upserts
    expect(players.map(p => canonicalJson(p.avatar.config))).toEqual(expected.map(c => canonicalJson(c)))
    expect(players.map(p => p.avatar.revision)).toEqual([1, 2])
    expect(canonicalJson(decoded)).toBe(canonicalJson(input))
  })
  it.each([
    { ...wholeManifest.presets[0]!.config, preset_id: 'photo01_front_07' },
    { ...wholeManifest.presets[0]!.config, item_ids: layered.item_ids },
    { ...wholeManifest.presets[0]!.config, contract_version: '1.0.0' },
    { ...layered, contract_version: '1.1.0' },
  ])(`C05 ${type} rejects invalid or mixed avatar config %#`, config => {
    expect(() => decode(JSON.stringify(frame(type, [config])), 'server')).toThrow(WorldTransportError)
  })
}
