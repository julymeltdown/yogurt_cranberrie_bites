// Common admission projection of the existing Root RuntimeQualificationPort.
// Root supplies the already parsed GateConfig and qualified inspect adapter.
// This does not load credentials, launch a profile, control faults or accept a case.
export const contractPins = Object.freeze({
  revision: '83f55f9639273a7994a30d6a591e642f7e4b674f',
  domain: '31a167fedd6b414a9d0b48df0f18644fcbd9f7a7a9bcef855265601e1401d634',
  realtime: '6ec593955a695656bcf5ec736e25fdf26593f1eff28bda657541779a1ec8cd85',
})
const unavailable = () => { throw new Error('QA09_MANAGED_ADMISSION_UNAVAILABLE') }
const need = value => { if (!value) unavailable() }
const uuid = value => typeof value === 'string' && value.length === 36
  && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
  && value !== '00000000-0000-0000-0000-000000000000'
const sha = value => typeof value === 'string' && value.length === 40 && /^[0-9a-f]+$/.test(value)
const services = ['gateway', 'auth', 'profile', 'concert', 'world', 'postgres']
function selection(config) {
  need(config?.version === 1 && config.isolated === true
    && config.allowDestructiveFixtureEndState === true && sha(config.releaseRevision)
    && uuid(config.environmentId) && uuid(config.eventId)
    && config.contractHashes?.domain === contractPins.domain
    && config.contractHashes?.realtime === contractPins.realtime)
  const origin = new URL(config.origin)
  need(origin.origin === config.origin && (origin.protocol === 'https:'
    || origin.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(origin.hostname)))
  need(Array.isArray(config.members) && config.members.length === 2)
  const actors = [...config.members.map(member => ({ userId: member.userId, role: 'MEMBER' })),
    { userId: config.operator?.userId, role: 'OPERATOR' }]
  need(actors.every(actor => uuid(actor.userId)) && new Set(actors.map(actor => actor.userId)).size === 3)
  return { candidate: config.releaseRevision, environmentId: config.environmentId,
    eventId: config.eventId, origin: config.origin, actors }
}

/** Use as observeRecoveryCase.guard on the existing managed page. The supplied
 * adapter must already be source-qualified by the runtime owner. A callback or
 * passing source fixture cannot establish that provenance or business evidence.
 * Root owns interruption/cleanup of an in-flight inspect socket at lease end.
 * No new schema, descriptor, listener, runtime, retry or restart is introduced.
 */
export function createManagedAdmissionGuard({ config, runtime, page, signal, deadlineEpochMs }) {
  try {
    const captured = selection(config), binding = JSON.stringify(captured)
    need(runtime && typeof runtime.inspect === 'function' && page && typeof page.url === 'function'
      && signal instanceof AbortSignal && Number.isFinite(deadlineEpochMs)
      && deadlineEpochMs > Date.now() && deadlineEpochMs <= Date.now() + 900_000)
    // Capture the method too: replacing an owner's adapter cannot silently swap custody.
    const inspect = runtime.inspect.bind(runtime)
    let cutoff = deadlineEpochMs
    const check = () => {
      need(!signal.aborted && Date.now() < cutoff && JSON.stringify(selection(config)) === binding
        && new URL(page.url()).origin === captured.origin)
    }
    return async function guard() {
      try {
        check()
        const proof = await inspect(config)
        check()
        need(proof?.isolated === true && proof.legacyData === false && proof.exclusiveEvent === true
          && proof.candidate === captured.candidate && proof.environmentId === captured.environmentId
          && proof.eventId === captured.eventId && proof.domainHash === contractPins.domain
          && proof.realtimeHash === contractPins.realtime)
        need(typeof proof.expiresAt === 'string' && proof.expiresAt.endsWith('Z'))
        const expiry = Date.parse(proof.expiresAt), now = Date.now()
        need(Number.isFinite(expiry) && expiry > now && expiry <= now + 900_000)
        // A later observer response can shorten this window, never renew it.
        cutoff = Math.min(cutoff, expiry)
        need(Array.isArray(proof.actors) && proof.actors.length === 3
          && new Set(proof.actors.map(actor => actor.userId)).size === 3)
        for (const actor of captured.actors) need(proof.actors.some(actual =>
          actual.userId === actor.userId && actual.role === actor.role))
        need(Array.isArray(proof.services) && proof.services.length === services.length
          && new Set(proof.services.map(service => service.name)).size === services.length)
        for (const name of services) need(proof.services.some(service =>
          service.name === name && service.ready === true && service.candidate === captured.candidate))
        check()
        // No actor IDs, credentials, origin, raw proof or diagnostic text escapes.
        return { managedObservationBound: true, contractRevision: contractPins.revision,
          businessCaseQualified: false, wholeQa09Accepted: false }
      } catch { unavailable() }
    }
  } catch { unavailable() }
}
