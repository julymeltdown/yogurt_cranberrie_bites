import { isAbsolute } from 'node:path'
import { parseServingConfiguration, bindRuntimePacket } from '../serve.mjs'
import { admitEnrollment } from './enrollment.mjs'
import { isRevision } from './source.mjs'

export const cases = ['feed-read', 'feed-comment', 'intro-unavailable', 'people-unavailable', 'sharing-unavailable', 'notification-unavailable', 'social-ui']
const fail = () => { throw Error('NOT_RUN: default application inputs unavailable') }
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value)
export function parseDefaultConfiguration(value) {
  const keys = ['version', 'servingConfig', 'runtimePacket', 'servedRoot', 'harnessRevision', 'publishedRevision', 'cases', 'timeoutMs']
  if (!record(value) || Object.keys(value).length !== keys.length || keys.some(key => !Object.hasOwn(value, key))
    || value.version !== 'mcr-default-app/v2' || ![value.servingConfig, value.runtimePacket, value.servedRoot].every(path => typeof path === 'string' && isAbsolute(path))
    || !isRevision(value.harnessRevision) || !isRevision(value.publishedRevision)
    || !Array.isArray(value.cases) || !value.cases.length || new Set(value.cases).size !== value.cases.length
    || value.cases.some(name => !cases.includes(name)) || value.cases.includes('feed-comment') && !value.cases.includes('feed-read')
    || !Number.isSafeInteger(value.timeoutMs) || value.timeoutMs < 10_000 || value.timeoutMs > 180_000) return fail()
  return Object.freeze({ ...value, cases: Object.freeze([...value.cases]) })
}

/** Correlated input preflight only. Publication and protected-read qualification come from owners. */
export function bindDefaultInputs(input, servingValue, packet, now = Date.now(), starting = true) {
  const serving = parseServingConfiguration(servingValue, now)
  if (serving.runtimePacket !== input.runtimePacket || serving.frontendRevision !== input.publishedRevision) return fail()
  const cutoff = bindRuntimePacket(serving, packet, now)
  const app = packet?.defaultApp
  if (!record(app) || Object.keys(app).sort().join(',') !== 'applicationRelease,frontendRevision,localInput,localInputRevision'
    || app.frontendRevision !== input.publishedRevision || !isRevision(app.localInputRevision) || !record(app.localInput)) return fail()
  const release = app.applicationRelease
  if (!record(release) || Object.keys(release).sort().join(',') !== 'applicationVersion,buildId,sourceRevision,version'
    || release.version !== 1 || typeof release.applicationVersion !== 'string' || typeof release.buildId !== 'string'
    || !/^[a-f0-9]{64}$/.test(release.buildId) || release.sourceRevision !== input.publishedRevision) return fail()
  // Reuse exact canonical Auth provenance, but never the old runner's identity/controller entry.
  if (!admitEnrollment({ origin: serving.origin, eventId: packet.event_id,
    runtime: packet.runtime, principals: packet.principals, authProducer: packet.authProducer }, now)) return fail()
  if (input.cases.includes('feed-comment') && (typeof packet.resources?.postId !== 'string'
    || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(packet.resources.postId))) return fail()
  let social = null
  if (input.cases.includes('social-ui')) {
    social = packet.resources?.in03Social
    const handle = value => typeof value === 'string' && (social.platform === 'X' ? /^[A-Za-z0-9_]{1,15}$/.test(value)
      : /^[A-Za-z0-9_](?:[A-Za-z0-9._]{0,28}[A-Za-z0-9_])?$/.test(value) && !value.includes('..'))
    if (!record(social) || Object.keys(social).sort().join(',') !== 'introId,platform,recipientHandle,requesterHandle'
      || typeof social.introId !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(social.introId)
      || !['INSTAGRAM', 'X'].includes(social.platform) || !handle(social.requesterHandle) || !handle(social.recipientHandle)
      || social.requesterHandle === social.recipientHandle
      || input.cases.some(name => name.endsWith('-unavailable'))) return fail()
  }
  if (starting && cutoff - now < input.timeoutMs + 10_000) return fail()
  return { serving, packet, cutoff, eventId: packet.event_id, localInput: app.localInput, applicationRelease: release,
    localInputRevision: app.localInputRevision, principals: packet.principals, resources: packet.resources ?? {}, social }
}

/** The actual frozen application's validator must approve this exact producer-supplied value. */
export function localInputDefine(value, validate) {
  if (typeof validate !== 'function' || !validate(value)) return fail()
  const raw = JSON.stringify(value)
  if (raw.length > 4096) return fail()
  return JSON.stringify(raw)
}
