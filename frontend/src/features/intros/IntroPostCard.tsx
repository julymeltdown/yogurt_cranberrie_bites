import { Link } from 'react-router-dom'
import { SocialIcon } from '../../shared/ui'
import { IntroLikeButton } from './IntroLikeButton'
import type { LikeTransport } from './api/intro-likes'
import type { IntroPostDetail } from './contract'

/**
 * WP-05 T-413: the intro post card.
 *
 * A card shows the body excerpt, the favorites snapshot, the attachment count,
 * the heart and the comment count.
 *
 * **There is no room preview and no room link anywhere in this component.** The
 * room is a separate place inside the profile; a post is never a doorway to it.
 * The import boundary is asserted by a test: this file imports nothing from
 * `features/profile-room/`, and neither does the detail.
 */
export interface IntroPostCardProps {
  readonly post: IntroPostDetail
  readonly transport: LikeTransport
  /** How many body characters the excerpt keeps. */
  readonly excerpt?: number
  readonly onUnavailable?: () => void
}

export const DEFAULT_EXCERPT = 180

export function excerptOf(body: string, limit: number = DEFAULT_EXCERPT): string {
  const characters = [...body]
  if (characters.length <= limit) return body
  return `${characters.slice(0, limit).join('')}\u2026`
}

export function IntroPostCard({ post, transport, excerpt = DEFAULT_EXCERPT, onUnavailable }: IntroPostCardProps) {
  return (
    <article className="intro-card" aria-labelledby={`intro-card-${post.intro_id}`}>
      <header className="intro-card-author">
        <span className="intro-card-avatar" aria-hidden="true">
          {post.author.nickname.slice(0, 1).toUpperCase()}
        </span>
        <Link to={`/profiles/${post.author.profile_id}`}>{post.author.nickname}</Link>
      </header>
      <p className="intro-card-body">{excerptOf(post.body, excerpt)}</p>
      {post.favorites_snapshot.length > 0 && (
        <ul className="intro-card-favorites" aria-label="Favorites">
          {post.favorites_snapshot.slice(0, 5).map((item) => (
            <li key={item.catalog_item_id}>{item.label}</li>
          ))}
        </ul>
      )}
      <footer className="intro-card-footer">
        <IntroLikeButton
          introId={post.intro_id}
          initial={{ count: post.like.count, liked: post.like.liked_by_viewer }}
          transport={transport}
          {...(onUnavailable === undefined ? {} : { onUnavailable })}
        />
        <span className="intro-card-comments">
          <SocialIcon name="comment" className="pi-icon--inline" />
          {post.comment_count} comments
        </span>
        <span className="intro-card-attachments">
          <SocialIcon name="photo" className="pi-icon--inline" />
          {post.attachments.length} photographs
        </span>
        <Link to={`/intro-posts/${post.intro_id}`}>Open post</Link>
      </footer>
    </article>
  )
}
