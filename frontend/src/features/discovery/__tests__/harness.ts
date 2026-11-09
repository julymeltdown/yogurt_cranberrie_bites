import { createTransportHarness } from '../../../test-support/harness'
import { createDiscoveryController } from '../controller'

/**
 * The shared harness for the discovery tests: the shared transport harness (a
 * real memory session, a real privacy scope, the real HTTP client with an
 * injected fetch) plus the real controller. Nothing here is a mock of the code
 * under test.
 */
export { ACCOUNT as userId } from '../../../test-support/harness'

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
  const transport = createTransportHarness({
    respond: async () => Response.json(peoplePage([card('Fallback', [sharedItem('MUSIC', 'Fallback Band')])])),
  })
  const controller = createDiscoveryController({ auth: { privacy: transport.privacy }, request: transport.http.request })
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
