/**
 * The one set of test scaffolding the feature suites share.
 *
 * Eleven work packages were written in parallel, and each one grew its own copy
 * of the same four helpers. The copies had already drifted — some `deferred`
 * returned `{ promise, resolve }`, some `{ promise, resolve, reject }`, one had a
 * different resolve parameter name — so a test that needed a rejection had to
 * know which copy it was importing.
 *
 * This module is the shared version. It is test-only: nothing under `src/`
 * production code imports it, and it imports no production module except the
 * types it needs.
 */

/**
 * A promise whose settlement the test controls.
 *
 * `reject` is always present, so a test that needs a failure does not have to
 * reach for a different helper. Existing call sites that only destructure
 * `{ promise, resolve }` keep working unchanged.
 */
export function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}

/** A promise the test resolves later, for a read that must stay pending. */
export type Deferred<T> = ReturnType<typeof deferred<T>>

/** A canonical lower-case UUID with a stable numeric tail. */
export const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`

/** The account every social fixture is bound to. */
export const ACCOUNT = id(3)
/** An event id for the fixtures that still need one. */
export const EVENT = id(1)

/** A canonical JSON response, as the transport expects one. */
export const json = (value: unknown, status = 200) =>
  Response.json(value, { status, headers: { 'content-type': 'application/json' } })

/** A closed error envelope with the given status and reason. */
export const failure = (status: number, reason: string, family = 'PROFILE') =>
  json({ code: `${family}_${status}`, message: 'Request unavailable', details: { reason } }, status)

/** The closed envelope shape a `TransportError` carries. */
export function errorEnvelope(status: number, reason: string, family = 'PROFILE') {
  const envelope = { code: `${family}_${status}`, message: 'Request unavailable', details: { reason } }
  return { envelope, core: { reason }, reason }
}

/** Wait for the microtask queue to drain, for a handler that must not win. */
export const settle = async () => {
  for (let index = 0; index < 8; index += 1) await Promise.resolve()
}
