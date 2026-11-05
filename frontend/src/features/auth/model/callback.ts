import { safeReturnTarget } from '../../../app/routes'

function localTarget(params: URLSearchParams) {
  return params.getAll('returnTo').length === 1 ? safeReturnTarget(params.get('returnTo')) : '/world'
}
export function readResetLocation(search: string, hash: string) {
  if (search.length > 16_384 || hash.length > 16_384) return { target: '/world', token: null, invalid: true }
  const params = new URLSearchParams(search), target = localTarget(params)
  // Source PasswordResetLinkBaseUrl emits BASE#token=<43 base64url characters>.
  // Query tokens, extra/duplicate fragment fields and malformed links are not imported.
  const match = /^#token=([A-Za-z0-9_-]{43})$/.exec(hash)
  const invalid = [...params.keys()].some(key => key !== 'returnTo') || params.getAll('returnTo').length > 1 || Boolean(hash && (!match || match[0] !== hash))
  return { target, token: !invalid && match ? match[1]! : null, invalid }
}
export type CallbackPresentation = { kind: 'denied' | 'invalid-state' | 'unavailable' | 'handoff' | 'restricted'; target: string }
export function readCallbackLocation(search: string, hash: string): CallbackPresentation {
  if (search.length > 16_384 || hash.length > 16_384) return { kind: 'invalid-state', target: '/world' }
  const params = new URLSearchParams(search), target = localTarget(params)
  if (params.has('status')) {
    const handoffTarget = params.get('returnTo')
    const safeTarget = ['/world', '/feed', '/me', '/meet/people'].includes(handoffTarget ?? '') ? handoffTarget! : '/world'
    if (hash || params.getAll('status').length !== 1 || params.getAll('returnTo').length > 1
      || [...params.keys()].some(key => key !== 'status' && key !== 'returnTo')) return { kind: 'invalid-state', target: safeTarget }
    const status = params.get('status')
    return { kind: status === 'success' ? 'handoff' : status === 'denied' ? 'denied' : status === 'error' ? 'unavailable' : 'invalid-state', target: safeTarget }
  }
  // Inspect only the denial marker/presence of fields. Never parse/import credentials,
  // replay a code, validate server state in the browser, or manufacture signed-in success.
  if (hash || params.getAll('returnTo').length > 1 || params.getAll('error').length > 1 || params.getAll('state').length > 1
    || [...params.keys()].some(key => !['error', 'state', 'returnTo', 'error_description', 'error_uri'].includes(key))) return { kind: 'invalid-state', target }
  if (params.get('error') === 'access_denied') return { kind: 'denied', target }
  if (params.has('state') || params.has('error')) return { kind: 'invalid-state', target }
  return { kind: 'unavailable', target }
}
