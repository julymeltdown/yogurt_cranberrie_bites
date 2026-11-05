import { useState, useSyncExternalStore, type ReactNode } from 'react'
import type { EventFeedController } from './controller'
import type { Filter, Notice, Post, ResourceLink } from './contract'
import { messages, type Slot } from './runtime'
import { Tabs } from '../../shared/ui/Tabs'
import { FeedGlyph } from './FeedGlyph'
import { AuthorHeading } from './parade-ink/AuthorHeading'
import './feed.css'

export function ReadStatus({ slot, retry }: { slot: Slot<unknown>; retry: () => void }) {
  if (slot.phase === 'loading') return <p role="status">Loading…</p>
  if (!slot.failure) return null
  return <div role="alert"><p>{messages[slot.failure]}</p><button type="button" onClick={retry}>Retry read</button></div>
}
export function NoticeContent({ notice }: { notice: Notice }) {
  return <article className="feed-notice-detail"><h2>{notice.title}</h2><p className="feed-text">{notice.text}</p><p><a href={notice.source_uri} target="_blank" rel="noopener noreferrer" referrerPolicy="no-referrer">Source</a> · Confirmed <time dateTime={notice.confirmed_at}>{new Date(notice.confirmed_at).toLocaleString()}</time></p></article>
}
export type RenderPostMedia = (mediaId: string, post: Post) => ReactNode
/** Caller resolves this exact public AvatarConfig through its reviewed, scoped renderer. */
export type RenderPostAuthor = (author: Post['author']) => ReactNode
export function PostContent({ post, onLink, renderMedia, renderAuthor }: {
  post: Post; onLink: (link: ResourceLink) => void; renderMedia?: RenderPostMedia | undefined; renderAuthor?: RenderPostAuthor | undefined
}) {
  return <>
    <AuthorHeading author={post.author} createdAt={post.created_at} edited={post.edited_at !== null} renderAuthor={renderAuthor} />
    <p className="feed-text feed-post-copy">{post.text}</p>
    {post.media_ids.length > 0 && (renderMedia ? <div className="feed-media" aria-label="Post images">{post.media_ids.map(id => <figure key={id}>{renderMedia(id, post)}</figure>)}</div> : <p className="feed-media-unavailable">{post.media_ids.length} image attachment(s). Image viewing is currently unavailable.</p>)}
    {post.links.length > 0 && <ul className="feed-related" aria-label="Related content">{post.links.map((link, i) => <li key={`${link.target_id}-${i}`}><button type="button" onClick={() => onLink(link)}>Open {link.target_type.toLowerCase()} <span aria-hidden="true">↗</span></button></li>)}</ul>}
  </>
}
export function EventFeed({ controller, onPost, onCompose, onLink, renderMedia, renderAuthor }: {
  renderMedia?: RenderPostMedia | undefined; renderAuthor?: RenderPostAuthor | undefined
  controller: EventFeedController; onPost: (id: string) => void; onCompose: () => void; onLink: (link: ResourceLink) => void
}) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const [notices, showNotices] = useState(false)
  const selected = notices ? 'NOTICE' : state.filter === 'WITH_MEDIA' ? 'ALL' : state.filter
  const posts = <>
    <ReadStatus slot={state.feed} retry={() => { void controller.loadFeed() }} />
    {state.feed.data?.items.length === 0 && <div className="feed-empty" role="status"><p>No posts yet.</p><p>Try another filter or come back later.</p>{state.notices.data?.items.length === 0 && <p>No published notices.</p>}</div>}
    <ol className="feed-list feed-posts" aria-label="Latest posts">{state.feed.data?.items.map(post => <li key={post.post_id}><article className={`feed-post ${post.media_ids.length > 0 ? 'feed-post--media' : 'feed-post--text'}`}>
      <PostContent post={post} onLink={onLink} renderMedia={renderMedia} renderAuthor={renderAuthor} />
      <div className="feed-post-actions" aria-label={`Actions for ${post.author.display_name}'s post`}>
        <button type="button" className={post.reacted_by_me ? 'feed-reacted' : ''} aria-label={`View ${post.reaction_count} reactions in thread`} onClick={() => onPost(post.post_id)}><FeedGlyph name="star" /><span>{post.reaction_count}</span></button>
        <button type="button" aria-label={`Open thread · ${post.comment_count} comments`} onClick={() => onPost(post.post_id)}><FeedGlyph name="comment" /><span>{post.comment_count}</span></button>
        <button type="button" aria-label="View post sharing in thread" onClick={() => onPost(post.post_id)}><FeedGlyph name="share" /></button>
      </div>
    </article></li>)}</ol>
    {state.feed.data?.next_cursor && <button className="feed-older" type="button" disabled={state.feed.phase === 'loading'} onClick={() => { void controller.loadFeed(true) }}>Load older posts</button>}
  </>
  const noticeList = <div className="feed-notices">
    <ReadStatus slot={state.notices} retry={() => { void controller.loadNotices() }} />
    {state.notices.data?.items.length === 0 && <p className="feed-empty" role="status">No published notices.</p>}
    <ul className="feed-list">{state.notices.data?.items.map(notice => <li key={notice.notice_id}><button type="button" className="feed-notice-link" onClick={() => { void controller.selectNotice(notice.notice_id) }}>{notice.title}</button><p>Confirmed <time dateTime={notice.confirmed_at}>{new Date(notice.confirmed_at).toLocaleString()}</time></p></li>)}</ul>
    {state.notices.data?.next_cursor && <button type="button" disabled={state.notices.phase === 'loading'} onClick={() => { void controller.loadNotices(true) }}>More notices</button>}
    <ReadStatus slot={state.notice} retry={() => { void controller.loadNotices() }} />{state.notice.data && <NoticeContent notice={state.notice.data} />}
  </div>
  return <section className="event-feed feed-browse" aria-label="Event feed">
    {!state.available ? <p className="feed-empty" role="status">Feed unavailable for the current account or event.</p> : <>
      {state.readOnly && <p className="feed-readonly" role="status">This event is read-only.</p>}
      <Tabs label="Post filters" value={selected} onValueChange={value => {
        showNotices(value === 'NOTICE')
        if (value !== 'NOTICE' && value !== state.filter) controller.setFilter(value as Filter)
      }} items={[
        { value: 'ALL', label: 'ALL', content: selected === 'ALL' ? posts : null },
        { value: 'PARTY_LINK', label: 'MEETUP', content: selected === 'PARTY_LINK' ? posts : null },
        { value: 'GIVEAWAY_LINK', label: 'GIVEAWAY', content: selected === 'GIVEAWAY_LINK' ? posts : null },
        { value: 'NOTICE', label: 'NOTICE', content: notices ? noticeList : null },
      ]} />
      <footer className="feed-footer">
        <details className="feed-tools"><summary aria-label="Feed options"><FeedGlyph name="options" /></summary><div>
          <label><input type="checkbox" checked={state.filter === 'WITH_MEDIA'} onChange={event => { showNotices(false); controller.setFilter(event.target.checked ? 'WITH_MEDIA' : 'ALL') }} /> Images only</label>
          <p>Reactions, comments and sharing open the thread.</p>
        </div></details>
        {!state.readOnly ? <button className="feed-compose" type="button" aria-label="Write a post" onClick={onCompose}><FeedGlyph name="plus" /> NEW POST</button> : <span className="feed-archive">READ-ONLY FEED</span>}
        <button className="feed-refresh" type="button" aria-label={notices ? 'Refresh notices' : 'Refresh feed'} disabled={(notices ? state.notices : state.feed).phase === 'loading'} onClick={() => { if (notices) void controller.loadNotices(); else void controller.loadFeed() }}><FeedGlyph name="refresh" /></button>
      </footer>
    </>}
  </section>
}

export function EventNotice({ controller, retry }: { controller: EventFeedController; retry: () => void }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  return <section className="event-feed" aria-label="Notice"><h1>NOTICE</h1><ReadStatus slot={state.notice} retry={retry} />{state.notice.data && <NoticeContent notice={state.notice.data} />}</section>
}
