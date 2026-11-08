import { QueryClient } from '@tanstack/react-query'
import { createMemorySession, createHttpClient } from '../../../shared/api/http'
import { createQueryPrivacy } from '../../../shared/query/privacy'
import { createDiscoveryController } from '../controller'

/**
 * The shared harness for the discovery tests: a real memory session, a real
 * privacy scope, the real HTTP client with an injected fetch, and the real
 * controller. Nothing here is a mock of the code under test.
 */
export const userId = '00000000-0000-4000-8000-000000000003'

export function sharedItem(category: 'MUSIC' | 'ANIME' | 'GAME', label: string) {
  return { catalog_item_id: crypto.randomUUID(), label, category, external_url: null }
}

export function card(
  nickname: string,
  shared: ReturnType<typeof sharedItem>[],
  options: { interaction_style?: string | null; profile_id?: string } = {},
) {
  return {
    profile_id: options.profile_id ?? crypto.randomUUID(),
    nickname,
    profile_image_media_id: null,
    shared_items: shared,
    interaction_style: options.interaction_style ?? null,
  }
}

export function peoplePage(items: ReturnType<typeof card>[], next: string | null = null) {
  return { items, next_cursor: next }
}

export function discoveryHarness() {
  const session = createMemorySession()
  session.replace(0, { userId, eventId: null, accessToken: 'fixture-token' })
  const query = new QueryClient()
  const privacy = createQueryPrivacy({ client: query, session })
  const requests: Request[] = []
  let respond: (request: Request) => Promise<Response> = async () =>
    Response.json(peoplePage([card('Fallback', [sharedItem('MUSIC', 'Fallback Band')])]))
  const http = createHttpClient({
    session,
    fetch: async (input) => {
      const request = input as Request
      requests.push(request.clone())
      return respond(request)
    },
  })
  const controller = createDiscoveryController({ auth: { privacy }, request: http.request })
  return {
    session,
    privacy,
    requests,
    controller,
    respond(value: typeof respond) {
      respond = value
    },
    dispose() {
      controller.dispose()
      privacy.dispose()
      query.clear()
    },
  }
}
