/**
 * The shared "real transport, stubbed fetch" harness the feature suites use.
 *
 * `discovery/__tests__/harness.ts`, `favorites/__tests__/harness.ts`,
 * `people/__tests__/fixtures/harness.ts` and `profile-media/__tests__/fixtures.ts`
 * each built the same four things by hand:
 *
 * 1. a real `createMemorySession` bound to one account;
 * 2. a real `createQueryPrivacy` coordinator over a real `QueryClient`;
 * 3. a real `createHttpClient` with an injected `fetch` that records every
 *    canonical `Request`;
 * 4. a `respond` setter so a test can change the answer mid-flight.
 *
 * Only `fetch` is stubbed, so every assertion a suite makes about a path, a
 * header or a body is an assertion about what the production client sends. This
 * module is that composition, once.
 *
 * It is test-only: nothing under production `src/` imports it.
 */
import { QueryClient } from '@tanstack/react-query'
import { createHttpClient, createMemorySession } from '../shared/api/http'
import { createQueryPrivacy, type PrivacyScope } from '../shared/query/privacy'

/** The account every harness is bound to unless a test names another. */
export const ACCOUNT = '00000000-0000-4000-8000-000000000003'

export type Respond = (request: Request) => Response | Promise<Response>

export interface HarnessOptions {
  /** The signed-in account. Defaults to {@link ACCOUNT}. */
  readonly userId?: string
  /** The event scope. `null` for a profile-only surface. */
  readonly eventId?: string | null
  /** The answer every request gets until a test replaces it. */
  readonly respond?: Respond
}

/**
 * Compose a real session, privacy coordinator and HTTP client over a stubbed
 * `fetch`.
 *
 * The returned `requests` holds a clone of every canonical `Request`, so a test
 * can read the body after the client has consumed it.
 */
export function createTransportHarness(options: HarnessOptions = {}) {
  const session = createMemorySession()
  session.replace(0, {
    userId: options.userId ?? ACCOUNT,
    eventId: options.eventId ?? null,
    accessToken: 'fixture-token',
  })
  const client = new QueryClient()
  const privacy = createQueryPrivacy({ client, session })
  const requests: Request[] = []
  let respond: Respond = options.respond ?? (async () => Response.json({}))
  const http = createHttpClient({
    session,
    fetch: async (input) => {
      const request = input as Request
      requests.push(request.clone())
      return respond(request)
    },
  })
  return {
    session,
    privacy,
    http,
    requests,
    /** The current privacy scope, as a controller would capture it. */
    scope: (): PrivacyScope => privacy.capture(),
    respond(next: Respond) {
      respond = next
    },
    dispose() {
      privacy.dispose()
      client.clear()
    },
  }
}

export type TransportHarness = ReturnType<typeof createTransportHarness>
