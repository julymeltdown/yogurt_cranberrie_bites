export function admitHTTP(config: unknown): { status: string; claim: string; missing: { input: string; owner: string }[]; IN03: string; unavailable: { input: string; owner: string }[] }
export function readRunConfig(path?: string): any
export function admitOriginal(config: unknown): ReturnType<typeof admitHTTP>
/** Private local-mail provenance preflight only; does not certify live HTTP or provider authority. */
export function admitLocalAuthProducer(config: unknown, now?: number): boolean
export interface LocalRealAuthProducer {
  version: 'mcr-local-real-auth/v1'
  kind: 'CANONICAL_GATEWAY_PRIVATE_HTTP_MAIL'
  sourceRevision: string
  binaryRevision: string
  tool: 'world_in01_prepare.py'
  mailOwner: string
  controlOwner: string
  origin: string
  eventId: string
  issuedAt: string
  expiresAt: string
  custody: 'PRIVATE_MODE_0600'
  actors: Record<'requester' | 'recipient' | 'restricted', {
    userId: string; eventId: string; email: string
    method: 'GATEWAY_SIGNUP_HTTP_MAIL_VERIFY_LOGIN'
    signupStatus: 200; verifyStatus: 200; loginStatus: 200; bootstrapStatus: 200
  }>
}
export interface LocalRealAuthRuntime {
  controlOwner: string; origin: string; eventId: string; expiresAt: string; private: true
}
