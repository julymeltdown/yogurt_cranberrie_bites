import { readFile } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { expect, it } from 'vitest'
import original from './source-inputs/operator-bootstrap.json'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import { eligibleAttendeeFixture, eligibleAttendeeScenario } from './eligible-attendee.mjs'
import { inspectInputReadiness } from './input-readiness.mjs'

const scope = { generation: 1, eventId: original.event.event_id, userId: original.participant.user_id }
it('preserves the raw disabled bootstrap as a decoded negative input, not constructor admission', async () => {
  const decoded = await decodeBootstrap(original, scope)
  expect(decoded.worldAvailable).toBe(false)
  expect(inspectInputReadiness(original)).toMatchObject({ status: 'INPUT_NOT_READY', productionAdmitted: false,
    reasons: ['WORLD_NOT_AVAILABLE'], worldReason: 'FEATURE_DISABLED' })
})
it('pins the frozen public capability schema, existing positive fixture source and original Map4 body', async () => {
  for (const [path, sha] of Object.entries(eligibleAttendeeScenario.sources)) {
    const root = resolve(process.cwd(), '..')
    const frozen = execFileSync('git', ['show', eligibleAttendeeScenario.source_revision + ':' + path], { cwd: root })
    expect(createHash('sha256').update(frozen).digest('hex'), path).toBe(sha)
    // The exclusive FE checkout is sparse; do not materialize backend paths to check provenance.
    const current = await readFile(path.startsWith('backend/')
      ? resolve(process.cwd(), 'src/app/world-entry/__tests__/map-presentation-v1/source-inputs/operator-bootstrap.json')
      : resolve(root, path))
    expect(current, path).toEqual(frozen)
  }
  expect(eligibleAttendeeScenario.projection).toEqual({ 'capabilities.world': { available: true, reason: null } })
  expect(eligibleAttendeeScenario.qualification).toBe('WORLD_POSITIVE_CONTRACT_FIXTURE_NOT_OPERATOR_OR_LIVE_ADMISSION')
})
it('fully decodes the explicit eligible fixture and changes only the fixture World capability', async () => {
  const input = eligibleAttendeeFixture()
  const decoded = await decodeBootstrap(input, scope)
  expect(decoded.worldAvailable).toBe(true)
  expect(inspectInputReadiness(input)).toMatchObject({ status: 'INPUT_CANDIDATE', productionAdmitted: false, reasons: [] })
  const restored = structuredClone(input)
  restored.capabilities.world = structuredClone(original.capabilities.world)
  expect(restored).toEqual(original)
  for (const key of ['map_manifest', 'theme_manifests', 'avatar_manifest', 'avatar'] as const)
    expect(JSON.stringify(input[key]), key).toBe(JSON.stringify(original[key]))
  expect(original.capabilities.world.available).toBe(false)
})
it('retains full C03 rejection of altered asset content and invalid positive capability reasons', async () => {
  const tampered = eligibleAttendeeFixture()
  tampered.map_manifest.geometry.collision_grid[0]![0] = 1 - tampered.map_manifest.geometry.collision_grid[0]![0]!
  await expect(decodeBootstrap(tampered, scope)).rejects.toThrow()
  const invalid = eligibleAttendeeFixture()
  invalid.capabilities.world.reason = 'FEATURE_DISABLED'
  expect(inspectInputReadiness(invalid).reasons).toContain('INVALID_POSITIVE_WORLD_REASON')
  await expect(decodeBootstrap(invalid, scope)).rejects.toThrow()
})
it('rejects absent, wrong-phase and wrong-map input without claiming full validation for enabled flags', () => {
  expect(inspectInputReadiness(null).status).toBe('INPUT_NOT_READY')
  const input = eligibleAttendeeFixture()
  input.event.phase = 'ARCHIVED'
  expect(inspectInputReadiness(input).reasons).toContain('EVENT_NOT_OPEN')
  input.map_manifest.map_revision = 7005
  expect(inspectInputReadiness(input).reasons).toContain('NOT_MAP4_INPUT')
})
