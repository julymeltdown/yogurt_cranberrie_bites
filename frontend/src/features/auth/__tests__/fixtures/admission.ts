import { admission } from '../enrollment-consent/fixtures'

/** Answer the canonical admission preflight outside each feature's request spy. */
export const withAdmission = (fetchImpl: typeof globalThis.fetch): typeof globalThis.fetch => async input => {
  const request = input as Request
  if (new URL(request.url).pathname === '/api/auth/enrollment/admission') return Response.json(admission())
  return fetchImpl(input)
}

/** Keep a synthetic identity admitted across replacement, as canonical Auth does for a live shell. */
export const autoAdmit = (auth: { read(): { admission?: string }; session: { read(): { userId: string | null } }; checkAdmission(): Promise<boolean>; subscribe(listener: () => void): () => void }) => {
  const sync = () => { if (auth.session.read().userId && auth.read().admission === 'unchecked') void auth.checkAdmission() }
  auth.subscribe(sync); sync()
}
