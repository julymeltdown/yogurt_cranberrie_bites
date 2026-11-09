import { createTransportHarness } from '../../../test-support/harness'
import { createFavoritesController } from '../controller'

/**
 * The shared harness for the favorites tests. It is the same shape the other
 * feature tests use: the shared transport harness (a real memory session, a real
 * privacy scope, the real HTTP client with an injected fetch) and a controller
 * built from it. Nothing here is a mock of the code under test.
 */
export { ACCOUNT as userId } from '../../../test-support/harness'

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
  const transport = createTransportHarness({
    respond: async (request) => {
      const url = new URL(request.url)
      const category = url.pathname.split('/')[3]?.toUpperCase() as 'MUSIC' | 'ANIME' | 'GAME'
      return Response.json(page(category, ['Fallback Band']))
    },
  })
  const controller = createFavoritesController({ auth: { privacy: transport.privacy }, request: transport.http.request })
  return {
    session: transport.session,
    privacy: transport.privacy,
    requests: transport.requests,
    controller,
    respond: transport.respond,
    dispose() {
      controller.dispose()
      transport.dispose()
    },
  }
}
