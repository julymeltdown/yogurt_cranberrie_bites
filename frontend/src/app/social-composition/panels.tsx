import { useSafetyEntry } from '../safety-entry/context'
import { useLayoutEffect, useRef, useSyncExternalStore } from 'react'
import { useLocation } from 'react-router-dom'
import { EventFeed, EventNotice, type RenderPostAuthor } from '../../features/event-feed'
import { RootPostImage } from '../media-entry/RootMediaView'
import { FeedThread, PostComposer } from '../../features/feed-thread'
import { Inbox } from '../../features/inbox'
import { People } from '../../features/people'
import { PublicProfile } from '../../features/profile'
import { ReviewedProfileAvatar } from '../../features/profile/Avatar'
import { WholeCharacterView } from '../../entities/avatar/whole-character'
import { useCharacterRuntime, useReviewedCharacterOwner } from '../character-entry/provider'
import { ReviewedCharacterView } from '../character-entry/ReviewedCharacterView'
import { useLoadedProfileActivity } from '../character-entry/activity'
import { IntroEditor } from '../../features/intro'
import { AgeStatus } from '../../features/age-status'
import { socialRoute } from './routes'
import { useSocialOwner } from './provider'
import type { SocialOwner } from './owner'
import type { SocialService } from './ports'
import './composition.css'
const noSubscribe = () => () => {}, empty = () => null
export function SocialRoutePanel() { return <SocialPanel path={useLocation().pathname} /> }
export function SocialPanel({ path }: { readonly path: string }) {
  const owner = useSocialOwner(), view = useSyncExternalStore(owner?.subscribe ?? noSubscribe, owner?.read ?? empty, empty)
  useLayoutEffect(() => { owner?.enter(path) }, [owner, path])
  if (!owner || !view || view.phase !== 'ready') return <Unavailable />
  const blocked = view.notice === 'command' || view.notice === 'draft'
  if (view.path !== path && !blocked) return <p role="status">Opening this section…</p>
  return <div className="social-composition">
    {view.notice && <div role="status"><p>{view.notice === 'command' ? 'Resolve the current action before leaving this panel. Its original command is retained.' : view.notice === 'draft' ? 'Keep or discard the current edit before changing its target. Your draft is retained.' : 'This destination is unavailable.'}</p>
      {blocked && view.path !== path && view.path && <button type="button" onClick={() => owner.navigate(view.path!)}>Return to current action</button>}</div>}
    <CurrentPanel owner={owner} path={view.path ?? path} />
  </div>
}
function Unavailable() { return <p role="status">This section is unavailable for the current account or event.</p> }
function CurrentPanel({ owner, path }: { readonly owner: SocialOwner; readonly path: string }) {
  const safety = useSafetyEntry()
  const route = socialRoute(path)
  const runtime = useCharacterRuntime(), reviewed = useReviewedCharacterOwner()
  const activity = useLoadedProfileActivity(route?.kind === 'profile' ? route.id : '')
  const service: SocialService | null = route ? ['feed', 'compose', 'thread', 'edit', 'notice'].includes(route.kind) ? 'feed' : route.kind === 'profile' ? 'people' : route.kind === 'handles' ? 'requests' : route.kind as SocialService : null
  const thread = owner.thread.read(), editEntry = useRef<{ owner: SocialOwner; path: string } | null>(null)
  useLayoutEffect(() => {
    if (route?.kind !== 'edit') { editEntry.current = null; return }
    if (editEntry.current?.owner === owner && editEntry.current.path === path || thread.selectedId !== route.id || thread.detail.phase !== 'ready') return
    // One initialization per entry, including a draft already started by the thread UI.
    // Discard/completion must not cause this route to begin the edit again.
    editEntry.current = { owner, path }
    if (!thread.draft.edit && !thread.draft.text) owner.thread.beginPostEdit()
  }, [owner, path, thread.selectedId, thread.detail, thread.draft])
  if (!route || !service || !owner.available(service)) return <Unavailable />
  const media = (id: string, post: { event_id: string }) => <RootPostImage auth={owner.auth} scope={owner.scope} createPreview={owner.media?.createPreview} mediaId={id} eventId={post.event_id} alt="Post attachment" />
  const renderAuthor: RenderPostAuthor | undefined = reviewed ? author => <ReviewedCharacterView owner={reviewed} config={author.avatar.config} label={`${author.display_name}'s avatar`} size={64} /> : runtime ? author => author.avatar.config.contract_version === '1.1.0'
    ? <WholeCharacterView runtime={runtime} config={author.avatar.config} label={`${author.display_name}'s avatar`} size={64} />
    : <span className="feed-initial" role="img" aria-label={`${author.display_name}: avatar unavailable`}>{Array.from(author.display_name.trim())[0]?.toLocaleUpperCase() ?? '?'}</span> : undefined
  switch (route.kind) {
    case 'feed': return <EventFeed controller={owner.feed} onPost={id => owner.navigate(`/feed/${id}`)} onCompose={() => owner.navigate('/feed/new')} onLink={link => { void owner.openResource(link) }} renderMedia={media}
      renderAuthor={renderAuthor} />
    case 'compose': return <PostComposer controller={owner.thread} onPost={id => owner.navigate(`/feed/${id}`)} />
    case 'thread': return <FeedThread controller={owner.threadPanel} onLink={link => { void owner.openResource(link) }} onEditPost={() => owner.navigate(`/feed/${route.id}/edit`)} onReport={target => owner.navigate(`/report?targetType=${target.target_type}&targetId=${target.target_id}`)} renderMedia={media} renderAuthor={renderAuthor} />
    case 'edit': return thread.draft.edit?.id === route.id ? <PostComposer controller={owner.thread} onPost={id => owner.navigate(`/feed/${id}`)} /> : <div role="status"><p>{thread.detail.phase === 'ready' ? 'Editing has ended or is unavailable for this post.' : 'Reading the current authorized post before editing. Editing is unavailable until ownership and write access are confirmed.'}</p><button type="button" onClick={() => owner.navigate(`/feed/${route.id}`)}>Back to post</button></div>
    case 'notice': return <EventNotice controller={owner.feed} retry={() => { void owner.feed.selectNotice(route.id) }} />
    case 'people': return <People controller={owner.people} avatars={owner.avatars} />
    case 'profile': return <PublicProfile controller={owner.people} userId={route.id}
      {...(owner.canShare(route.id) ? { sharing: (id: string) => { void owner.openSharing(id) } } : {})}
      {...(safety && route.id !== owner.scope.userId ? { blocking: (id: string) => { void safety.beginBlock(id, owner.scope) } } : {})} avatars={owner.avatars} activity={activity} activityPrivacy={owner.auth.privacy}
      renderAvatar={reviewed ? (avatar, label) => <ReviewedCharacterView owner={reviewed} config={avatar.config} label={label} size={64} /> : runtime ? (avatar, label) => avatar.config.contract_version === '1.1.0'
        ? <WholeCharacterView runtime={runtime} config={avatar.config} label={label} size={64} />
        : <ReviewedProfileAvatar config={avatar.config} label={label} options={owner.avatars} /> : undefined} />
    case 'intro': return <IntroEditor controller={owner.intro} />
    case 'age': return <AgeStatus controller={owner.age} />
    case 'inbox': return <Inbox controller={owner.inbox} />
    case 'handles': case 'requests': { const module = owner.requests(); return module ? <module.Panel /> : <Unavailable /> }
  }
}
/** Canonical observation only. Unavailable or stale count is null, never loaded-row arithmetic. */
export function useSocialUnreadTotal() {
  const owner = useSocialOwner(), view = useSyncExternalStore(owner?.subscribe ?? noSubscribe, owner?.read ?? empty, empty)
  const count = owner?.inbox.read().count
  return owner && view?.phase === 'ready' && owner.available('inbox') && count?.phase === 'ready' ? count : null
}
