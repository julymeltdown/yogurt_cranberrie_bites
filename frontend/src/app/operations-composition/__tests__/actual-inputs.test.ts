// TEST ONLY input boundaries, never a deployment, producer or actual Admin proof.
import { expect, it } from 'vitest'
import { parseConfig } from '../../../../e2e/real/mural-helpers'
import { ownedInput, validateBackendObservation, validateFrontendHandoff, validateFrontendObservation } from './actual-inputs'
const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12,'0')}`
const config = parseConfig({ version: 1, isolated: true, allowDestructiveFixtureEndState: true,
  origin: 'http://127.0.0.1:52624', eventId: id(1), releaseRevision: 'a'.repeat(40), environmentId: id(2),
  members: [3,4].map(n => ({ userId: id(n), email: `TEST_ONLY_${n}@example.com`, password: 'TEST_ONLY_password' })),
  operator: { userId: id(5), email: 'TEST_ONLY_operator@example.com', password: 'TEST_ONLY_password' },
  contractHashes: { domain: 'b'.repeat(64), realtime: 'c'.repeat(64) }, restoreSnapshotId: id(6), maxCooldownMs: 5000, restartObservationTimeoutMs: 1000 })
const handoff = () => ({ candidate: config.releaseRevision, environmentId: config.environmentId, eventId: config.eventId,
  origin: config.origin, settingsSha256: 'd'.repeat(64), token: 'e'.repeat(64), phaserModulePath: '/@fs/private/tmp/TEST_ONLY/deps/phaser.js' })
it('requires private actual paths rather than manufacturing a default runtime input', async () => {
  await expect(ownedInput(undefined, true)).rejects.toThrow('ACTUAL_PRIVATE_INPUT_REQUIRED')
})
it('rejects foreign, expired and changed frontend bindings before any browser', () => {
  const h = validateFrontendHandoff(config, handoff()), now = Date.now()
  const live = { ...h, expiresAt: new Date(now + 50000).toISOString(), phaserModuleUrl: h.origin + h.phaserModulePath + '?v=abc' }
  expect(() => validateFrontendObservation(h, live, now)).not.toThrow()
  expect(() => validateFrontendHandoff(config, { ...h, eventId: id(99) })).toThrow('FRONTEND_SCOPE_MISMATCH')
  expect(() => validateFrontendObservation(h, { ...live, expiresAt: new Date(now).toISOString() }, now)).toThrow('FRESH_FRONTEND_OBSERVATION_REQUIRED')
  expect(() => validateFrontendObservation(h, { ...live, settingsSha256: 'f'.repeat(64) }, now)).toThrow('CURRENT_FRONTEND_BINDING_REQUIRED')
})
it('rejects observed backend scope and actor substitutions; config labels alone supply no authority', () => {
  const now = Date.now(), proof = { candidate: config.releaseRevision, environmentId: config.environmentId, eventId: config.eventId,
    domainHash: config.contractHashes.domain, realtimeHash: config.contractHashes.realtime, expiresAt: new Date(now + 50000).toISOString(),
    actors: [...config.members.map(a => ({ userId: a.userId, role: 'MEMBER' })), { userId: config.operator.userId, role: 'OPERATOR' }] }
  expect(() => validateBackendObservation(config, proof, now)).not.toThrow()
  expect(() => validateBackendObservation(config, { ...proof, eventId: id(99) }, now)).toThrow('CURRENT_BACKEND_SCOPE_REQUIRED')
  expect(() => validateBackendObservation(config, { ...proof, actors: proof.actors.map(a => ({ ...a, role: 'OPERATOR' })) }, now)).toThrow('OBSERVED_ACTOR_BINDING_REQUIRED')
})
