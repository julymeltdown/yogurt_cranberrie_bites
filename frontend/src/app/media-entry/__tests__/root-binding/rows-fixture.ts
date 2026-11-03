import { fixture, post, response, event, id } from '../../../social-composition/__tests__/fixtures'
import { giveaway } from '../../../../features/gift-discovery/__tests__/fixtures/data'
export { event }
export const postMedia = id(51), giftMedia = id(52)
export const authorizedPost = { ...post, media_ids: [postMedia] }
export const authorizedGift = giveaway({ event_id: event, image_media_id: giftMedia, title: 'Authorized synthetic giveaway' })
export const belowFoldPosts = Array.from({ length: 7 }, (_, n) => ({ ...post, post_id: id(100 + n), created_at: new Date(Date.parse(post.created_at) - (n + 1) * 60_000).toISOString(), text: `Authorized below-fold row ${n}`, media_ids: [id(200 + n)] }))
export function rowFixture(readMedia: (request: Request) => Promise<Response>, feedEnabled = true, extraRows = false) {
  const f = fixture(r => {
    const path = new URL(r.url).pathname
    if (path.includes('/media/')) return readMedia(r)
    if (path.endsWith('/giveaways')) return Response.json({ items: [authorizedGift], next_cursor: null })
    if (path.endsWith('/feed')) return Response.json({ items: [authorizedPost, ...(extraRows ? belowFoldPosts : [])], next_cursor: null })
    if (path.includes('/posts/')) return Response.json({ post: authorizedPost, comments: [], next_cursor: null })
    return response(r)
  })
  const ports = { ...f.ports, bootstrap: { load: async () => f.bootstrap }, social: {
    availability(context: { scope: typeof f.scope; bootstrap: typeof f.bootstrap }) {
      const value = { scope: context.scope, bootstrap: context.bootstrap, services: { feed: feedEnabled } }
      return { read: () => value, subscribe: () => () => undefined }
    },
  } }
  return { ...f, ports }
}
