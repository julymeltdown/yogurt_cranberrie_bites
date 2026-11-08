import { useState } from 'react'
import { Link } from 'react-router-dom'
import { IntroLikeButton } from './IntroLikeButton'
import type { LikeTransport } from './api/intro-likes'
import type { IntroComment, IntroPostDetail as Detail } from './contract'

/**
 * WP-05 T-413: the intro post detail.
 *
 * The full body, the attachments, the comments, the heart and the author link.
 *
 * **No room preview anywhere.** This component imports nothing from
 * `features/profile-room/`, and it offers no room action: opening the room is a
 * separate action inside the profile.
 *
 * A restricted post renders the neutral prompt rather than partial content. The
 * detail is only constructed from a post the server already decided the viewer
 * may see, so the "restricted" case is the server's `404`, which the caller
 * passes in as `unavailable`.
 */
export interface IntroPostDetailProps {
  readonly post: Detail | null
  readonly comments: readonly IntroComment[]
  readonly transport: LikeTransport
  /** Set when the server refused the post. The detail then shows the prompt. */
  readonly unavailable?: boolean
  readonly onUnavailable?: () => void
}

export const RESTRICTED_PROMPT = 'This post is not available to you.'

export function IntroPostDetail({ post, comments, transport, unavailable = false, onUnavailable }: IntroPostDetailProps) {
  const [expanded, setExpanded] = useState(false)
  if (unavailable || post === null) {
    return (
      <section className="intro-detail" aria-label="Intro post">
        <p role="status" className="intro-detail-unavailable">
          {RESTRICTED_PROMPT}
        </p>
      </section>
    )
  }
  return (
    <section className="intro-detail" aria-labelledby={`intro-detail-${post.intro_id}`}>
      <header className="intro-detail-author">
        <span className="intro-card-avatar" aria-hidden="true">
          {post.author.nickname.slice(0, 1).toUpperCase()}
        </span>
        <Link to={`/profiles/${post.author.profile_id}`}>{post.author.nickname}</Link>
      </header>
      <h1 id={`intro-detail-${post.intro_id}`}>Mutuals-intro post</h1>
      <p className="intro-detail-body">{post.body}</p>
      {post.currently_seeking !== null && post.currently_seeking !== '' && (
        <p className="intro-detail-seeking">Looking for: {post.currently_seeking}</p>
      )}
      {post.interaction_style !== null && post.interaction_style !== '' && (
        <p className="intro-detail-style">{post.interaction_style}</p>
      )}
      {post.favorites_snapshot.length > 0 && (
        <ul className="intro-card-favorites" aria-label="Favorites">
          {post.favorites_snapshot.map((item) => (
            <li key={item.catalog_item_id}>{item.label}</li>
          ))}
        </ul>
      )}
      {post.attachments.length > 0 && (
        <ul className="intro-detail-attachments" aria-label="Photographs">
          {post.attachments.map((media) => (
            <li key={media.media_id}>
              <img alt="" src={media.read_path} width={media.width} height={media.height} decoding="async" loading="lazy" />
            </li>
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
        <span>{post.comment_count} comments</span>
      </footer>
      <section className="intro-detail-comments" aria-label="Comments">
        <h2>Comments</h2>
        {comments.length === 0 ? (
          <p>No comments yet.</p>
        ) : (
          <ul>
            {comments.map((comment) => (
              <li key={comment.comment_id} className={comment.parent_comment_id === null ? 'intro-comment' : 'intro-comment intro-comment-reply'}>
                <Link to={`/profiles/${comment.author.profile_id}`}>{comment.author.nickname}</Link>
                <p>{comment.deleted ? 'This comment was deleted.' : comment.body}</p>
              </li>
            ))}
          </ul>
        )}
        <button type="button" onClick={() => setExpanded((value) => !value)} aria-expanded={expanded}>
          {expanded ? 'Hide comment box' : 'Write a comment'}
        </button>
        {expanded && (
          <p className="intro-detail-note">
            Comments are visible only to people who can see this post.
          </p>
        )}
      </section>
    </section>
  )
}
