/** Bounded metadata only: never store HTTP bodies, cookies, credentials, handles or auth frames. */
export function publicPath(url) {
  const path = new URL(url).pathname
  return path.replace(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g, '{id}')
}
export function policyMetadata(payload, expectedEvent, activeStream, expectedUser) {
  try {
    if (typeof payload !== 'string' || payload.length > 1024 * 1024) return null
    const value = JSON.parse(payload)
    if (value?.type === 'snapshot') return value.protocolVersion === '1.0.0' && value.eventId === expectedEvent
      && value.selfUserId === expectedUser && typeof expectedUser === 'string' && value.streamSeq === 0
      && typeof value.streamId === 'string' && value.streamId.length > 0 && value.streamId.length <= 128
      ? { type: 'snapshot', streamId: value.streamId, matches: true } : null
    const keys = value?.type === 'policy.baseline' ? ['protocolVersion', 'type', 'streamId', 'eventId', 'scope', 'policyRevision']
      : value?.type === 'policy.invalidate' ? ['protocolVersion', 'type', 'streamId', 'eventId', 'scope', 'policyRevision', 'reason'] : []
    if (!keys.length || Object.keys(value).length !== keys.length || keys.some(key => !Object.hasOwn(value, key))
      || value.protocolVersion !== '1.0.0' || value.scope !== 'SOCIAL' || value.eventId !== expectedEvent
      || typeof value.streamId !== 'string' || !Number.isSafeInteger(value.policyRevision) || value.policyRevision < 1
      || value.type === 'policy.invalidate' && !['SOURCE_CHANGED', 'CONSENT_CHANGED', 'BLOCK_CHANGED', 'EXPIRED', 'CANONICAL_POLICY_CHANGED'].includes(value.reason)) return null
    return { type: value.type, streamId: value.streamId, revision: value.policyRevision,
      matches: activeStream !== null && value.streamId === activeStream,
      ...(value.type === 'policy.invalidate' ? { reason: value.reason } : {}) }
  } catch { return null }
}
export function createAudit(limit = 2000) {
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > 10_000) throw Error('Invalid audit limit')
  const rows = []; let sequence = 0, overflow = false
  const keys = ['kind', 'method', 'path', 'status', 'requestedAt', 'receivedAt', 'sha256', 'sourceSha256', 'matches', 'revision']
  return {
    add(row) {
      if (row === null || typeof row !== 'object' || Array.isArray(row) || Object.keys(row).some(key => !keys.includes(key))) throw Error('Unsafe audit field')
      if (rows.length >= limit) { overflow = true; return }
      rows.push(Object.freeze({ sequence: ++sequence, ...row }))
    },
    read: () => ({ rows: [...rows], overflow }),
  }
}
