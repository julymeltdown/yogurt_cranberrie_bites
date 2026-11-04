// Consumer of the frozen bd087 Enrollment producer, not an identity/provider issuer.
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value)
const keys = (value, names) => object(value) && Object.keys(value).length === names.length && names.every(name => Object.hasOwn(value, name))
const revision = value => typeof value === 'string' && /^[0-9a-f]{40}$/.test(value)
const uuid = value => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value) && !value.startsWith('00000000-')
const roles = ['requester', 'recipient', 'restricted']
export function admitEnrollment(config, now = Date.now()) {
  try {
    const { authProducer: p, runtime: r, principals } = config
    if (!keys(p, ['version', 'kind', 'sourceRevision', 'binaryRevision', 'tool', 'mailOwner', 'controlOwner', 'origin', 'eventId', 'issuedAt', 'expiresAt', 'custody', 'actors', 'operatorPolicyProductionApproval'])
      || !keys(r, ['private', 'controlOwner', 'origin', 'eventId', 'expiresAt']) || !keys(principals, roles) || !keys(p.actors, roles)) return false
    if (p.version !== 'mcr-local-real-auth/v1' || p.kind !== 'CANONICAL_ENROLLMENT_GATEWAY_PRIVATE_HTTP_MAIL'
      || p.tool !== 'session_deletion_runtime/enrollment.py' || p.custody !== 'PRIVATE_MODE_0600' || p.operatorPolicyProductionApproval !== false
      || !revision(p.sourceRevision) || !revision(p.binaryRevision) || !uuid(config.eventId) || r.private !== true) return false
    const url = new URL(config.origin)
    if (!['http:', 'https:'].includes(url.protocol) || !['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname)
      || url.username || url.password || url.pathname !== '/' || url.search || url.hash
      || p.origin !== config.origin || r.origin !== config.origin || p.eventId !== config.eventId || r.eventId !== config.eventId
      || typeof p.controlOwner !== 'string' || p.controlOwner.length < 16 || p.controlOwner.length > 256
      || p.mailOwner !== p.controlOwner || r.controlOwner !== p.controlOwner || r.expiresAt !== p.expiresAt) return false
    const utc = value => typeof value === 'string' && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,6})?(?:Z|\+00:00)$/.test(value) && Number.isFinite(Date.parse(value))
    if (!utc(p.issuedAt) || !utc(p.expiresAt) || !Number.isFinite(now) || Date.parse(p.issuedAt) > now
      || Date.parse(p.expiresAt) <= now || Date.parse(p.expiresAt) <= Date.parse(p.issuedAt)) return false
    const users = new Set(), emails = new Set()
    for (const role of roles) {
      const actor = p.actors[role], login = principals[role]
      if (!keys(actor, ['userId', 'email', 'eventId', 'method', 'signupStatus', 'verifyStatus', 'loginStatus', 'bootstrapStatus'])
        || !keys(login, ['userId', 'email', 'password']) || !uuid(actor.userId) || actor.userId !== login.userId
        || actor.email !== login.email || typeof actor.email !== 'string' || !/^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@example\.test$/.test(actor.email)
        || /fixture|synthetic|mock/i.test(actor.email) || typeof login.password !== 'string' || !login.password.length
        || actor.eventId !== config.eventId || actor.method !== 'GATEWAY_ENROLLMENT_COOKIE_HTTP_MAIL_VERIFY_LOGIN'
        || actor.signupStatus !== 202 || actor.verifyStatus !== 200 || actor.loginStatus !== 200 || actor.bootstrapStatus !== 200) return false
      users.add(actor.userId); emails.add(actor.email.toLowerCase())
    }
    return users.size === 3 && emails.size === 3
  } catch { return false }
}
