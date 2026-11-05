import { useState, useSyncExternalStore } from 'react'
import type { FeedThreadController } from './controller'
import type { ResourceLink } from '../event-feed/contract'
import { PostContent, ReadStatus, type RenderPostAuthor, type RenderPostMedia } from '../event-feed/EventFeed'
import { messages } from '../event-feed/runtime'
import { AuthorHeading } from '../event-feed/parade-ink/AuthorHeading'
import { FeedGlyph } from '../event-feed/FeedGlyph'
import '../event-feed/feed.css'

function WriteStatus({ controller, onPost }: { controller: FeedThreadController; onPost?: (id: string) => void }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), mutation = state.mutation
  if (mutation.phase === 'idle') return null
  return <div className="feed-command-status" data-phase={mutation.phase} aria-live="polite" aria-busy={mutation.phase === 'sending'}><p>{mutation.phase === 'sending' ? 'Sending…' : mutation.phase === 'uncertain' ? 'The outcome is unknown. Your draft and command identity are retained. Check status before any new submission.' : mutation.phase === 'pending' ? 'The server is processing this command. Check status explicitly.' : mutation.phase === 'succeeded' ? 'The server confirmed this change.' : mutation.failure ? messages[mutation.failure] : 'The change failed.'}</p>
    {['pending', 'uncertain'].includes(mutation.phase) && <button type="button" onClick={() => { void controller.checkStatus() }}>Check command status</button>}
    {mutation.phase === 'succeeded' && mutation.operation === 'createConcertPost' && mutation.resourceId && onPost && <button type="button" onClick={() => onPost(mutation.resourceId!)}>Open published post</button>}
  </div>
}
const noSubscribe = () => () => undefined
const noUpload = () => null
export function PostComposer({ controller, onPost }: { controller: FeedThreadController; onPost: (id: string) => void }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const upload = useSyncExternalStore(controller.media?.subscribe ?? noSubscribe, controller.media?.read ?? noUpload, noUpload)
  const busy = ['sending', 'uncertain', 'pending'].includes(state.mutation.phase), disabled = busy || !state.available || state.readOnly
  const uploadPending = upload !== null && !['idle', 'cancelled'].includes(upload.phase)
  return <section className="event-feed feed-composer" aria-label="Post composer"><h1>{state.draft.edit ? 'EDIT POST' : 'NEW POST'}</h1>
    {!state.available && <p role="status">Posting is unavailable for this account or event.</p>}{state.readOnly && <p role="status">This event is read-only.</p>}
    <form className="feed-compose-form" onSubmit={e => { e.preventDefault(); void controller.submitPost() }}>
      <label htmlFor="post-text">Post text</label><textarea id="post-text" value={state.draft.text} disabled={disabled} onChange={e => controller.setPostText(e.target.value)} aria-describedby="post-limit" />
      <p id="post-limit" className="feed-character-count">{Array.from(state.draft.text).length} / 4000 characters</p>
      {controller.media && upload ? <fieldset disabled={disabled}><legend>Images</legend>
        <label>Select an image <input type="file" accept="image/jpeg,image/png,image/webp" disabled={uploadPending || state.draft.media_ids.length >= 4} onChange={e => { const file = e.target.files?.[0]; if (file) void controller.media!.select(file); e.target.value = '' }} /></label>
        {upload.previewUrl?.startsWith('blob:') && <img className="feed-upload-preview" src={upload.previewUrl} alt="Selected image preview" />}
        <p role="status">{upload.phase === 'ready' ? 'Upload verified. Add the image before publishing.' : upload.phase === 'uncertain' ? 'Upload outcome unknown. Check status; do not upload again.' : upload.phase === 'failed' ? 'Image preparation or upload failed. Your text is retained.' : `Image: ${upload.phase}`}</p>
        {upload.phase === 'prepared' && <button type="button" onClick={() => { void controller.media!.upload() }}>Upload selected image</button>}
        {upload.phase === 'uncertain' && <button type="button" onClick={() => { void controller.media!.recover() }}>Check image status</button>}
        {upload.phase === 'failed' && <button type="button" onClick={() => { void controller.media!.retry() }}>Retry image preparation</button>}
        {upload.phase === 'ready' && <button type="button" onClick={() => { controller.attachReadyMedia() }}>Add uploaded image</button>}
        {uploadPending && <button type="button" onClick={() => controller.media!.cancel()}>Discard selected image</button>}
      </fieldset> : <p>Image upload is currently unavailable. You can publish text.</p>}
      {state.draft.media_ids.length > 0 && <ul aria-label="Attached images">{state.draft.media_ids.map((id, index) => <li key={id}>Image {index + 1} <button type="button" disabled={disabled} onClick={() => controller.removeMedia(id)}>Remove image {index + 1}</button></li>)}</ul>}
      <button className="feed-submit" type="submit" disabled={disabled || uploadPending || !state.draft.text.trim() || Array.from(state.draft.text).length > 4000}>{state.draft.edit ? 'Save post changes' : 'Publish post'}</button>
      {state.draft.edit && <button type="button" disabled={busy} onClick={controller.cancelPostEdit}>Discard edit</button>}
    </form><WriteStatus controller={controller} onPost={onPost} />
    {state.draft.edit && <button type="button" disabled={busy} onClick={() => { void controller.loadPost() }}>Reload current post</button>}
  </section>
}
export function FeedThread({ controller, onLink, onEditPost, onReport, renderMedia, renderAuthor }: {
  renderAuthor?: RenderPostAuthor | undefined
  renderMedia?: RenderPostMedia | undefined; controller: FeedThreadController; onLink: (link: ResourceLink) => void; onEditPost: () => void
  onReport: (target: { target_type: 'POST' | 'COMMENT'; target_id: string }) => void
}) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const [deletion, setDeletion] = useState<{ postId: string; commentId: string | null } | null>(null)
  const post = state.detail.data?.post, busy = ['sending', 'uncertain', 'pending'].includes(state.mutation.phase), disabled = busy || !state.available || state.readOnly
  return <section className="event-feed feed-thread" aria-label="Post thread"><h1>THREAD</h1>
    {!state.available && <p role="status">Thread unavailable for the current account or event.</p>}
    {state.readOnly && <p role="status">This event is read-only.</p>}<ReadStatus slot={state.detail} retry={() => { void controller.loadPost() }} />
    {post && <><article className={`feed-post ${post.media_ids.length > 0 ? 'feed-post--media' : 'feed-post--text'}`}><PostContent post={post} onLink={onLink} renderMedia={renderMedia} renderAuthor={renderAuthor} />
      <div className="feed-post-actions" aria-label="Post reactions and comments"><button type="button" aria-label={post.reacted_by_me ? 'Remove reaction' : 'React'} aria-pressed={post.reacted_by_me} disabled={disabled} onClick={() => { void controller.react() }}><FeedGlyph name="star" /><span>{post.reaction_count}</span></button><span className="feed-comment-count"><FeedGlyph name="comment" />{post.comment_count} comments</span></div>
      <div className="feed-content-actions" aria-label="Post controls">
      <button type="button" aria-label="Report post" onClick={() => onReport({ target_type: 'POST', target_id: post.post_id })}>Report</button>
      {post.author.user_id === state.userId && <><button type="button" disabled={disabled} aria-label="Edit post" onClick={() => { if (controller.beginPostEdit()) onEditPost() }}>Edit</button><button type="button" disabled={disabled} aria-label="Delete post" onClick={() => setDeletion({ postId: post.post_id, commentId: null })}>Delete</button></>}
      </div>
    </article>
    <section className="feed-comments" aria-label="Comments"><h2>Comments</h2><ReadStatus slot={state.comments} retry={() => { void controller.loadPost() }} />
      {state.comments.data?.items.length === 0 && <p>No comments yet.</p>}
      <ol className="feed-list feed-replies">{state.comments.data?.items.map(comment => <li className="feed-reply" key={comment.comment_id}>
        <AuthorHeading author={comment.author} createdAt={comment.created_at} renderAuthor={renderAuthor} /><p className="feed-text feed-reply-copy">{comment.text}</p>
        <div className="feed-content-actions" aria-label={`Comment controls for ${comment.author.display_name}`}>
        <button type="button" aria-label="Report comment" onClick={() => onReport({ target_type: 'COMMENT', target_id: comment.comment_id })}>Report</button>
        {comment.author.user_id === state.userId && <><button type="button" disabled={disabled} aria-label="Edit comment" onClick={() => { controller.beginCommentEdit(comment.comment_id); document.getElementById('reply-text')?.focus() }}>Edit</button><button type="button" disabled={disabled} aria-label="Delete comment" onClick={() => setDeletion({ postId: post.post_id, commentId: comment.comment_id })}>Delete</button></>}
        </div>
      </li>)}</ol>
      {state.comments.data?.next_cursor && <button type="button" disabled={state.comments.phase === 'loading'} onClick={() => { void controller.moreComments() }}>More comments</button>}
      <form className="feed-reply-form" onSubmit={e => { e.preventDefault(); void controller.submitComment() }}><label htmlFor="reply-text">{state.reply.edit ? 'Edit comment text' : 'Comment text'}</label>
        <textarea id="reply-text" value={state.reply.text} disabled={disabled} onChange={e => controller.setReplyText(e.target.value)} aria-describedby="reply-limit" /><p id="reply-limit" className="feed-character-count">{Array.from(state.reply.text).length} / 1000 characters</p>
        <button className="feed-submit" type="submit" disabled={disabled || !state.reply.text.trim() || Array.from(state.reply.text).length > 1000}>{state.reply.edit ? 'Save comment changes' : 'Send comment'}</button>
        {state.reply.edit && <button type="button" disabled={busy} onClick={controller.cancelCommentEdit}>Discard edit</button>}
      </form>
    </section>
    {deletion?.postId === post.post_id && <fieldset className="feed-delete-confirmation"><legend>Confirm deletion</legend><p>This will delete {deletion.commentId ? 'your comment' : 'this post'}.</p><button className="feed-submit" type="button" disabled={disabled} onClick={() => { if (deletion.commentId) void controller.deleteComment(deletion.commentId); else void controller.deletePost(); setDeletion(null) }}>Confirm deletion</button><button type="button" onClick={() => setDeletion(null)}>Keep content</button></fieldset>}
    </>}
    <WriteStatus controller={controller} />
  </section>
}
