import '../../../profile/reference-presentation/activity.css'
import { useCallback, useSyncExternalStore, type ReactNode } from 'react'
import { ShellLink } from '../../../../app/navigation'
import type { PrivacyScope } from '../../../../shared/query/privacy'
import type { SessionController } from '../../../auth/model/session'
import { ordered, parse, type Page, type Post } from '../../../event-feed/contract'
import { ConcertIcon } from '../../../../shared/ui/concert-window'
/** Presentation of an already-authorized bounded feed page, never a new query. */
export interface ProfileActivityInput {
  readonly scope: PrivacyScope
  readonly subjectUserId: string
  readonly page: Page<Post>
}
export type ActivityPrivacy = Pick<SessionController['privacy'], 'isCurrent' | 'subscribe'>
/** Root owns protected, already-loaded media and its asynchronous failure fallback. */
export type ProfilePostMediaRenderer = (post: Post) => ReactNode
const noSubscribe = () => undefined
export function qualifiedProfilePosts(input: ProfileActivityInput | undefined, subject: string, privacy: ActivityPrivacy | undefined): readonly Post[] | null {
  if (!input || !privacy || !privacy.isCurrent(input.scope) || input.subjectUserId !== subject || !input.scope.eventId) return null
  try {
    const page = parse<Page<Post>>('Domain_feed_FeedPage', input.page)
    if (!page.items.every(p => p.event_id === input.scope.eventId) || !ordered(page.items, p => p.post_id)) return null
    return page.items.filter(p => p.author.user_id === subject).slice(0, 2)
  } catch { return null }
}
function PostVisual({ post, renderPostMedia }: { post: Post; renderPostMedia?: ProfilePostMediaRenderer | undefined }) {
  let media: ReactNode = null
  // Never ask the renderer to discover media or render an unqualified post.
  if (post.media_ids[0] && renderPostMedia) {
    try { media = renderPostMedia(post) } catch { /* A failed presentation port keeps the ordinary icon. */ }
  }
  return <span className="reference-activity__visual" aria-hidden="true">{media || <ConcertIcon name="feed" />}</span>
}
export function ProfileActivity({ input, subject, privacy, browseFeed = false, renderPostMedia }: { input?: ProfileActivityInput | undefined; subject: string; privacy?: ActivityPrivacy | undefined; browseFeed?: boolean; renderPostMedia?: ProfilePostMediaRenderer | undefined }) {
  const subscribe = useCallback((notify: () => void) => {
    try { return privacy?.subscribe(notify) ?? noSubscribe } catch { return noSubscribe }
  }, [privacy])
  const current = useSyncExternalStore(subscribe, () => !!input && !!privacy?.isCurrent(input.scope), () => false)
  const posts = current ? qualifiedProfilePosts(input, subject, privacy) : null
  return <section className="reference-activity" aria-label="Recent profile activity">
    {posts === null ? <><p className="reference-activity__notice" role="status">Recent activity is unavailable in this connection.</p>{browseFeed && <ShellLink to="/feed" className="me-activity-load">Open feed</ShellLink>}</>
      : <><p className="reference-activity__scope">Recent posts from the loaded event feed page. This is a partial view of activity.</p>
        {posts.length ? <ul className="reference-activity__list">{posts.map(post => <li key={post.post_id}>
          <PostVisual post={post} renderPostMedia={renderPostMedia} />
          <div><ShellLink to={`/feed/${post.post_id}`} className="reference-activity__entry"><strong>Posted to the feed</strong><span className="reference-activity__text">{post.text}</span></ShellLink>
          <p><time dateTime={post.created_at}>{new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit', timeZone: 'UTC' }).format(new Date(post.created_at))} UTC</time><span> · {post.reaction_count} reactions · {post.comment_count} replies</span></p></div>
        </li>)}</ul> : <p className="reference-activity__notice">No posts by this profile in the loaded feed page.</p>}
      </>}
  </section>
}
