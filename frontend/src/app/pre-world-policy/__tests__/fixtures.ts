/** TEST ONLY known catalog pair; never a production release or recognition claim. */
export const policyFixtureRelease = Object.freeze({ version: 1 as const, applicationVersion: '0.1.0',
  buildId: 'a'.repeat(64), sourceRevision: 'b'.repeat(40) })
export function servicePolicyFixture(kind: 'NORMAL' | 'MAINTENANCE' | 'CLIENT_VERSION_REQUIRED' | 'UNAVAILABLE' = 'NORMAL',
  options: { serverTime?: number; ttl?: number; revision?: number; minimum?: string } = {}) {
  const time = options.serverTime ?? Date.now()
  return { event_revision: 1, policy_revision: kind === 'UNAVAILABLE' ? null : options.revision ?? 1, kind,
    minimum_client_version: kind === 'UNAVAILABLE' ? null : options.minimum ?? (kind === 'CLIENT_VERSION_REQUIRED' ? '0.2.0' : '0.1.0'),
    server_time: new Date(time).toISOString(), valid_until: new Date(time + (options.ttl ?? 5_000)).toISOString() }
}
