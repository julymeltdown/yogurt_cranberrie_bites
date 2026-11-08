import { QueryClient } from '@tanstack/react-query'
import { createMemorySession, createHttpClient } from '../../../shared/api/http'
import { createQueryPrivacy } from '../../../shared/query/privacy'
import { createFavoritesController } from '../controller'

/**
 * The shared harness for the favorites tests. It is the same shape the other
 * feature tests use: a real memory session, a real privacy scope, the real HTTP
 * client with an injected fetch, and a controller built from them. Nothing here
 * is a mock of the code under test.
 */
export const userId = '00000000-0000-4000-8000-000000000003'

export function catalogItem(category: 'MUSIC' | 'ANIME' | 'GAME', label: string, id?: string) {
  return {
    catalog_item_id: id ?? crypto.randomUUID(),
    label,
    category,
    external_url: null,
  }
}

export function page(category: 'MUSIC' | 'ANIME' | 'GAME', labels: string[], next: string | null = null) {
  return { category, items: labels.map((label) => catalogItem(category, label)), next_cursor: next }
}

export function favoritesHarness() {
  const session = createMemorySession()
  session.replace(0, { userId, eventId: null, accessToken: 'fixture-token' })
  const query = new QueryClient()
  const privacy = createQueryPrivacy({ client: query, session })
  const requests: Request[] = []
  let respond: (request: Request) => Promise<Response> = async (request) => {
    const url = new URL(request.url)
    const category = url.pathname.split('/')[3]?.toUpperCase() as 'MUSIC' | 'ANIME' | 'GAME'
    return Response.json(page(category, ['Fallback Band']))
  }
  const http = createHttpClient({
    session,
    fetch: async (input) => {
      const request = input as Request
      requests.push(request.clone())
      return respond(request)
    },
  })
  const controller = createFavoritesController({
    auth: { privacy },
    request: http.request,
  })
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
