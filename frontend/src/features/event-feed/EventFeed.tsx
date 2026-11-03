import { useSyncExternalStore, type ReactNode } from 'react'
import type { EventFeedController } from './controller'
import type { Filter, Notice, Post, ResourceLink } from './contract'
import { messages, type Slot } from './runtime'
import { ConcertAction, ConcertState, ConcertTabs, ConcertWindow } from '../../shared/ui/concert-window'
import './feed.css'

export function ReadStatus({ slot, retry }: { slot: Slot<unknown>; retry: () => void }) {
  if (slot.phase === 'loading') return <p role="status">Loading…</p>
  if (!slot.failure) return null
  return <div role="alert"><p>{messages[slot.failure]}</p><button type="button" onClick={retry}>Retry read</button></div>
}
export function NoticeContent({ notice }: { notice: Notice }) {
  return <article><h2>{notice.title}</h2><p className="feed-text">{notice.text}</p><p><a href={notice.source_uri} target="_blank" rel="noopener noreferrer" referrerPolicy="no-referrer">Source</a> · Confirmed <time dateTime={notice.confirmed_at}>{new Date(notice.confirmed_at).toLocaleString()}</time></p></article>
}
export type RenderPostMedia = (mediaId: string, post: Post) => ReactNode
export function PostContent({ post, onLink, renderMedia }: { post: Post; onLink: (link: ResourceLink) => void; renderMedia?: RenderPostMedia | undefined }) {
  return <><p><strong>{post.author.display_name}</strong> · <time dateTime={post.created_at}>{new Date(post.created_at).toLocaleString()}</time>{post.edited_at ? ' · Edited' : ''}</p>
    <p className="feed-text">{post.text}</p>
    {post.media_ids.length > 0 && (renderMedia ? <div aria-label="Post images">{post.media_ids.map(id => <figure key={id}>{renderMedia(id, post)}</figure>)}</div> : <p>{post.media_ids.length} image attachment(s). Image viewing is currently unavailable.</p>)}
    {post.links.length > 0 && <ul aria-label="Related content">{post.links.map((link, i) => <li key={`${link.target_id}-${i}`}><button type="button" onClick={() => onLink(link)}>Open {link.target_type.toLowerCase()}</button></li>)}</ul>}
  </>
}
export function EventFeed({ controller, onPost, onCompose, onLink, renderMedia }: {
  renderMedia?: RenderPostMedia | undefined; controller: EventFeedController; onPost: (id: string) => void; onCompose: () => void; onLink: (link: ResourceLink) => void
}) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const filters: { value: Filter; label: string }[] = [
    { value: 'ALL', label: 'All' }, { value: 'WITH_MEDIA', label: 'Images' },
    { value: 'PARTY_LINK', label: 'Parties' }, { value: 'GIVEAWAY_LINK', label: 'Gifts' },
  ]
  const posts = <>
    <ReadStatus slot={state.feed} retry={() => { void controller.loadFeed() }} />
    {state.feed.data?.items.length === 0 && <ConcertState title="No posts yet.">Try another filter or come back later.</ConcertState>}
    <ol className="feed-list feed-posts" aria-label="Latest posts">{state.feed.data?.items.map(post => <li key={post.post_id}><article>
      <PostContent post={post} onLink={onLink} renderMedia={renderMedia} />
      <ConcertAction tone="paper" onClick={() => onPost(post.post_id)}>Open thread · {post.comment_count} comments · {post.reaction_count} reactions</ConcertAction>
    </article></li>)}</ol>
    {state.feed.data?.next_cursor && <ConcertAction tone="paper" pending={state.feed.phase === 'loading'} onClick={() => { void controller.loadFeed(true) }}>Load older posts</ConcertAction>}
  </>
  return <section className="event-feed feed-browse" aria-label="Event feed"><ConcertWindow title="FEED" icon="feed" headingLevel={1}>
    {!state.available ? <ConcertState kind="unavailable" title="Feed unavailable">Feed unavailable for the current account or event.</ConcertState> : <>
      <div className="feed-toolbar">
        {state.available && !state.readOnly && <ConcertAction onClick={onCompose}>Write a post</ConcertAction>}
        <ConcertAction tone="paper" pending={state.feed.phase === 'loading'} onClick={() => { void controller.loadFeed() }}>Refresh feed</ConcertAction>
      </div>
      {state.readOnly && <ConcertState title="Read-only event">This event is read-only.</ConcertState>}
      <details className="feed-notices"><summary>Event notices{state.notices.data ? ` · ${state.notices.data.items.length}` : ''}</summary>
        <ReadStatus slot={state.notices} retry={() => { void controller.loadNotices() }} />
        {state.notices.data?.items.length === 0 && <p>No published notices.</p>}
        <ul className="feed-list">{state.notices.data?.items.map(notice => <li key={notice.notice_id}><button type="button" onClick={() => { void controller.selectNotice(notice.notice_id) }}>{notice.title}</button><p>Confirmed <time dateTime={notice.confirmed_at}>{new Date(notice.confirmed_at).toLocaleString()}</time></p></li>)}</ul>
        {state.notices.data?.next_cursor && <ConcertAction tone="paper" pending={state.notices.phase === 'loading'} onClick={() => { void controller.loadNotices(true) }}>More notices</ConcertAction>}
      </details>
      <ReadStatus slot={state.notice} retry={() => { void controller.loadNotices() }} />{state.notice.data && <NoticeContent notice={state.notice.data} />}
      <ConcertTabs label="Post filters" value={state.filter} onValueChange={value => controller.setFilter(value as Filter)}
        items={filters.map(filter => ({ ...filter, content: state.filter === filter.value ? posts : null }))} />
    </>}
  </ConcertWindow></section>
}

export function EventNotice({ controller, retry }: { controller: EventFeedController; retry: () => void }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  return <section className="event-feed" aria-label="Notice"><h1>NOTICE</h1><ReadStatus slot={state.notice} retry={retry} />{state.notice.data && <NoticeContent notice={state.notice.data} />}</section>
}
