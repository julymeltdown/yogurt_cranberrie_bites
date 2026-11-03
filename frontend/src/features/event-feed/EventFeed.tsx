import { useSyncExternalStore } from 'react'
import type { EventFeedController } from './controller'
import type { Filter, Notice, Post, ResourceLink } from './contract'
import { messages, type Slot } from './runtime'
import './feed.css'

export function ReadStatus({ slot, retry }: { slot: Slot<unknown>; retry: () => void }) {
  if (slot.phase === 'loading') return <p role="status">Loading…</p>
  if (!slot.failure) return null
  return <div role="alert"><p>{messages[slot.failure]}</p><button type="button" onClick={retry}>Retry read</button></div>
}
export function NoticeContent({ notice }: { notice: Notice }) {
  return <article><h2>{notice.title}</h2><p className="feed-text">{notice.text}</p><p><a href={notice.source_uri} target="_blank" rel="noopener noreferrer" referrerPolicy="no-referrer">Source</a> · Confirmed <time dateTime={notice.confirmed_at}>{new Date(notice.confirmed_at).toLocaleString()}</time></p></article>
}
export function PostContent({ post, onLink }: { post: Post; onLink: (link: ResourceLink) => void }) {
  return <><p><strong>{post.author.display_name}</strong> · <time dateTime={post.created_at}>{new Date(post.created_at).toLocaleString()}</time>{post.edited_at ? ' · Edited' : ''}</p>
    <p className="feed-text">{post.text}</p>
    {post.media_ids.length > 0 && <p>{post.media_ids.length} image attachment(s). Image viewing is currently unavailable.</p>}
    {post.links.length > 0 && <ul aria-label="Related content">{post.links.map((link, i) => <li key={`${link.target_id}-${i}`}><button type="button" onClick={() => onLink(link)}>Open {link.target_type.toLowerCase()}</button></li>)}</ul>}
  </>
}
export function EventFeed({ controller, onPost, onCompose, onLink }: {
  controller: EventFeedController; onPost: (id: string) => void; onCompose: () => void; onLink: (link: ResourceLink) => void
}) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  return <section className="event-feed" aria-label="Event feed"><header><p>From the crowd</p><h1>FEED</h1><p>Latest posts from this event.</p>
    {state.available && !state.readOnly && <button type="button" onClick={onCompose}>Write a post</button>}</header>
    {!state.available ? <p role="status">Feed unavailable for the current account or event.</p> : <>
      {state.readOnly && <p role="status">This event is read-only.</p>}
      <section aria-label="Event notices"><h2>Notices</h2><ReadStatus slot={state.notices} retry={() => { void controller.loadNotices() }} />
        {state.notices.data?.items.length === 0 && <p>No published notices.</p>}
        <ul className="feed-list">{state.notices.data?.items.map(notice => <li key={notice.notice_id}><button type="button" onClick={() => { void controller.selectNotice(notice.notice_id) }}>{notice.title}</button><p>Confirmed <time dateTime={notice.confirmed_at}>{new Date(notice.confirmed_at).toLocaleString()}</time></p></li>)}</ul>
        {state.notices.data?.next_cursor && <button type="button" disabled={state.notices.phase === 'loading'} onClick={() => { void controller.loadNotices(true) }}>More notices</button>}
      </section>
      <ReadStatus slot={state.notice} retry={() => { void controller.loadNotices() }} />{state.notice.data && <NoticeContent notice={state.notice.data} />}
      <label>Show <select value={state.filter} onChange={e => controller.setFilter(e.target.value as Filter)}><option value="ALL">All posts</option><option value="WITH_MEDIA">With images</option><option value="PARTY_LINK">Parties</option><option value="GIVEAWAY_LINK">Giveaways</option></select></label>
      <button type="button" disabled={state.feed.phase === 'loading'} onClick={() => { void controller.loadFeed() }}>Refresh feed</button>
      <ReadStatus slot={state.feed} retry={() => { void controller.loadFeed() }} />
      {state.feed.data?.items.length === 0 && <p>No posts yet.</p>}
      <ol className="feed-list" aria-label="Latest posts">{state.feed.data?.items.map(post => <li key={post.post_id}><article><PostContent post={post} onLink={onLink} /><button type="button" onClick={() => onPost(post.post_id)}>Open thread · {post.comment_count} comments · {post.reaction_count} reactions</button></article></li>)}</ol>
      {state.feed.data?.next_cursor && <button type="button" disabled={state.feed.phase === 'loading'} onClick={() => { void controller.loadFeed(true) }}>Load older posts</button>}
    </>}
  </section>
}
export function EventNotice({ controller, retry }: { controller: EventFeedController; retry: () => void }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  return <section className="event-feed" aria-label="Notice"><h1>NOTICE</h1><ReadStatus slot={state.notice} retry={retry} />{state.notice.data && <NoticeContent notice={state.notice.data} />}</section>
}
