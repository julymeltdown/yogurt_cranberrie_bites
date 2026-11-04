import { expect, it, vi } from 'vitest'
import bootstrap from '../../../../../../contracts/fixtures/core/bootstrap-open.json'
import selected from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import movement from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { decodeAvatarState, validateManifest } from '../../../../entities/avatar/whole-admission'
import { MOTION_V2_CANDIDATE } from '../../../../entities/avatar/runtime/motion-adoption/v2-candidate'
import { decodeMotionV2ConfigText } from '../../../../entities/avatar/runtime/motion-adoption/v2'
import { createSessionController } from '../../../../features/auth/model/session'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import { isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { decode } from '../../../../world/network/transport/contract'
import { bootstrapFor } from '../../../__tests__/fixtures/ports'
import { createRootCharacterRegistry } from '../../runtime'
import { readPinnedProducer } from '../../../../world/avatar/__tests__/motion-adoption/producer-inputs'

const producer = (path: string) => new TextDecoder().decode(readPinnedProducer(path))
const manifestText = producer(MOTION_V2_CANDIDATE.path), candidate = JSON.parse(manifestText)
const config = decodeMotionV2ConfigText(producer('contracts/fixtures/assets/motion-v2/valid/layered-max.json'))
const pin = { manifest_id: candidate.manifest_id, manifest_version: 1, content_hash: MOTION_V2_CANDIDATE.sha256 }

it('the actual application registry refuses the candidate and forged eligibility without allocating assets or borrowing a World owner', () => {
  const fetch = vi.fn(async () => { throw Error('No candidate network allocation is authorized') })
  const auth = createSessionController({ eventId: bootstrap.event.event_id, cookiePolicy: isolatedCookiePolicy(), fetch })
  auth.session.replace(auth.session.read().generation, { userId: bootstrap.participant.user_id, eventId: bootstrap.event.event_id, accessToken: 'TEST_ONLY_root_denial' })
  const scope = auth.privacy.capture(), root = createRootCharacterRegistry(auth, 'http://localhost')
  try {
    const shell = bootstrapFor({ generation: scope.generation, userId: bootstrap.participant.user_id, eventId: bootstrap.event.event_id })
    for (const manifest of [candidate, { ...candidate, content_hash: pin.content_hash },
      { ...candidate, production_eligible: true, qualification: 'ACCEPTED', content_hash: pin.content_hash }]) {
      expect(root.get(scope, { ...shell, document: { avatar_manifest: manifest } })).toBeNull()
      expect(root.read(scope, pin)).toBeNull()
    }
    const staticPin = { manifest_id: selected.manifest_id, manifest_version: 1, content_hash: selected.content_hash }
    const old = root.get(scope, { ...shell, document: { avatar_manifest: selected } })
    expect(old).not.toBeNull(); expect(root.read(scope, staticPin)).toBe(old)
    expect(root.read(scope, pin)).toBeNull()
    auth.privacy.changePolicy(scope, 'block')
    expect(old!.isCurrent()).toBe(false); expect(root.read(scope, staticPin)).toBeNull()
    expect(fetch).not.toHaveBeenCalled()
  } finally { root.dispose(); auth.dispose() }
})

it('C03, C05 and public avatar reads deny v2 until canonical schema generation and a separately admitted release exist', async () => {
  const state = { revision: 1, config }
  expect(() => decodeAvatarState(state)).toThrow()
  await expect(validateManifest(manifestText, pin)).rejects.toThrow()
  const scope = { generation: 1, userId: bootstrap.participant.user_id, eventId: bootstrap.event.event_id }
  const input = { ...bootstrap, avatar: state, avatar_manifest: candidate,
    participant: { ...bootstrap.participant, profile: { ...bootstrap.participant.profile, avatar: state } } }
  await expect(decodeBootstrap(input, scope)).rejects.toThrow()
  const snapshot = movement.messages.find(row => row.message.type === 'snapshot')!.message
  if (snapshot.type !== 'snapshot' || !('players' in snapshot)) throw Error('Missing immutable C05 snapshot')
  const frame = { ...snapshot, players: snapshot.players.map(player => ({ ...player, avatar: state })) }
  expect(() => decode(JSON.stringify(frame), 'server')).toThrow()
})
