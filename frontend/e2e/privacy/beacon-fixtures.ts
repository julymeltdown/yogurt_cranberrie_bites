import { harness, beaconId } from '../../src/features/meet-beacon/__tests__/fixtures/harness'
import { check, assertAbsent, deferred, markers, until, type Cell, type Role } from './oracle'

/** Uses the retained conspicuously synthetic public-port fixture; no venue/real audience grant. */
export async function beaconCells(role: Role): Promise<Cell[]> {
  const f = await harness(), initial = f.authority!
  const sample = { ...f.beacon, owner_id: role === 'member' ? '00000000-0000-4000-8000-000000000991' : f.beacon.owner_id,
    meeting: { ...f.beacon.meeting, instructions: markers[3] } }
  f.setBeacon(sample)
  const allowed = role === 'owner' || role === 'member'
  if (role === 'member') f.setAuthority({ ...initial, canWrite: false })
  else if (role === 'expired') f.setTime(Date.parse(sample.meeting.expires_at) + 1)
  else if (!allowed) f.setAuthority(null)
  try {
    const beaconReads = () => f.requests.filter(request => new URL(request.url).pathname.includes('/beacons/')).length
    await f.controller.select(beaconId)
    if (allowed) {
      check(f.controller.read().beacon?.meeting.instructions === markers[3], 'synthetic audience-scoped beacon detail visible')
      const late = deferred<Response>(), before = beaconReads()
      f.setOverride(request => new URL(request.url).pathname.includes('/beacons/') ? late.promise : null)
      const pending = f.controller.refresh()
      await until(() => beaconReads() > before)
      f.setAuthority(null); late.resolve(Response.json(sample)); await pending
      assertAbsent(f.controller.read())
      check(f.controller.read().beacon === null, 'late beacon cannot restore lost authority')
      f.setOverride(null); f.setAuthority({ ...initial, canWrite: role === 'owner' })
      await f.controller.select(beaconId)
      if (role === 'owner') {
        check(await f.controller.revoke(), 'owner explicitly revokes current beacon revision')
        const request = f.requests.find(request => request.method === 'DELETE')
        check(request && new URL(request.url).searchParams.get('expected_revision') === String(sample.revision), 'beacon revoke binds expected revision')
        check(f.controller.read().phase === 'revoked', 'beacon terminal state visible')
      } else {
        check(!f.controller.editCurrent() && await f.controller.revoke() === false, 'read-only audience member cannot author or revoke')
        check(!f.requests.some(request => request.method !== 'GET'), 'member performs no beacon mutation')
      }
    } else {
      check(f.controller.read().beacon === null && f.controller.read().photo === null, 'denied/expired beacon guidance hidden')
      check(await f.controller.revoke() === false, 'unavailable beacon cannot be revoked by stale client state')
      if (role !== 'expired') check(beaconReads() === 0, 'absent meeting authority prevents beacon read')
      assertAbsent(f.controller.read())
    }
    f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block')
    assertAbsent(f.controller.read())
    check(f.controller.read().beacon === null && f.controller.read().draft.instructions === '', 'beacon detail/draft scrubbed on actual privacy interface')
    return (['detail', 'mutation', 'cache'] as const).map(operation => ({ role, operation, status: 'PASS', provenance: 'SYNTHETIC_FIXTURE',
      evidence: 'Actual beacon and places controllers via retained synthetic port fixture; canRead/canWrite audience input, expiry, owner revision-bound revoke, read-only member, authority-loss late read and private draft cleanup. No actual venue/role policy/public WS/UI certification. Beacon list has no frozen operation in this harness.' }))
  } finally { f.dispose() }
}
