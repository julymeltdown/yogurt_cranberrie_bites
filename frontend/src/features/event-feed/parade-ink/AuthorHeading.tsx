import type { ReactNode } from 'react'
import type { Author } from '../contract'
import { relativePostTime } from '../presentation'

/** Presentation only: the caller retains the reviewed portrait and its lifetime. */
export function AuthorHeading({ author, createdAt, edited = false, renderAuthor }: {
  author: Author; createdAt: string; edited?: boolean
  renderAuthor?: ((author: Author) => ReactNode) | undefined
}) {
  return <header className="feed-post-heading">
    <div className="feed-portrait">{renderAuthor ? renderAuthor(author) : <span className="feed-initial" role="img" aria-label={`${author.display_name}: avatar unavailable`}>{Array.from(author.display_name.trim())[0]?.toLocaleUpperCase() ?? '?'}</span>}</div>
    <div className="feed-author-line"><strong>{author.display_name}</strong><time dateTime={createdAt} title={new Date(createdAt).toLocaleString()}>{relativePostTime(createdAt)}</time>{edited && <small>Edited</small>}</div>
  </header>
}
