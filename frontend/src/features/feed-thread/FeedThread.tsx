import { useState, useSyncExternalStore } from 'react'
import type { FeedThreadController } from './controller'
import type { ResourceLink } from '../event-feed/contract'
import { PostContent, ReadStatus, type RenderPostMedia } from '../event-feed/EventFeed'
import { messages } from '../event-feed/runtime'
import '../event-feed/feed.css'

function WriteStatus({ controller, onPost }: { controller: FeedThreadController; onPost?: (id: string) => void }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), mutation = state.mutation
  if (mutation.phase === 'idle') return null
  return <div aria-live="polite"><p>{mutation.phase === 'sending' ? 'Sending…' : mutation.phase === 'uncertain' ? 'The outcome is unknown. Your draft and command identity are retained. Check status before any new submission.' : mutation.phase === 'pending' ? 'The server is processing this command. Check status explicitly.' : mutation.phase === 'succeeded' ? 'The server confirmed this change.' : mutation.failure ? messages[mutation.failure] : 'The change failed.'}</p>
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
  return <section className="event-feed" aria-label="Post composer"><h1>{state.draft.edit ? 'EDIT POST' : 'NEW POST'}</h1>
    {!state.available && <p role="status">Posting is unavailable for this account or event.</p>}{state.readOnly && <p role="status">This event is read-only.</p>}
    <form onSubmit={e => { e.preventDefault(); void controller.submitPost() }}>
      <label htmlFor="post-text">Post text</label><textarea id="post-text" value={state.draft.text} disabled={disabled} onChange={e => controller.setPostText(e.target.value)} aria-describedby="post-limit" />
      <p id="post-limit">{Array.from(state.draft.text).length} / 4000 characters</p>
      {controller.media && upload ? <fieldset disabled={disabled}><legend>Images</legend>
        <label>Select an image <input type="file" accept="image/jpeg,image/png,image/webp" disabled={uploadPending || state.draft.media_ids.length >= 4} onChange={e => { const file = e.target.files?.[0]; if (file) void controller.media!.select(file); e.target.value = '' }} /></label>
        {upload.previewUrl?.startsWith('blob:') && <img src={upload.previewUrl} alt="Selected image preview" style={{ maxWidth: '100%', maxHeight: '12rem' }} />}
        <p role="status">{upload.phase === 'ready' ? 'Upload verified. Add the image before publishing.' : upload.phase === 'uncertain' ? 'Upload outcome unknown. Check status; do not upload again.' : upload.phase === 'failed' ? 'Image preparation or upload failed. Your text is retained.' : `Image: ${upload.phase}`}</p>
        {upload.phase === 'prepared' && <button type="button" onClick={() => { void controller.media!.upload() }}>Upload selected image</button>}
        {upload.phase === 'uncertain' && <button type="button" onClick={() => { void controller.media!.recover() }}>Check image status</button>}
        {upload.phase === 'failed' && <button type="button" onClick={() => { void controller.media!.retry() }}>Retry image preparation</button>}
        {upload.phase === 'ready' && <button type="button" onClick={() => { controller.attachReadyMedia() }}>Add uploaded image</button>}
        {uploadPending && <button type="button" onClick={() => controller.media!.cancel()}>Discard selected image</button>}
      </fieldset> : <p>Image upload is currently unavailable. You can publish text.</p>}
      {state.draft.media_ids.length > 0 && <ul aria-label="Attached images">{state.draft.media_ids.map((id, index) => <li key={id}>Image {index + 1} <button type="button" disabled={disabled} onClick={() => controller.removeMedia(id)}>Remove image {index + 1}</button></li>)}</ul>}
      <button type="submit" disabled={disabled || uploadPending || !state.draft.text.trim() || Array.from(state.draft.text).length > 4000}>{state.draft.edit ? 'Save post changes' : 'Publish post'}</button>
      {state.draft.edit && <button type="button" disabled={busy} onClick={controller.cancelPostEdit}>Discard edit</button>}
    </form><WriteStatus controller={controller} onPost={onPost} />
    {state.draft.edit && <button type="button" disabled={busy} onClick={() => { void controller.loadPost() }}>Reload current post</button>}
  </section>
}
export function FeedThread({ controller, onLink, onEditPost, onReport, renderMedia }: {
  renderMedia?: RenderPostMedia | undefined; controller: FeedThreadController; onLink: (link: ResourceLink) => void; onEditPost: () => void
  onReport: (target: { target_type: 'POST' | 'COMMENT'; target_id: string }) => void
}) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const [deletion, setDeletion] = useState<{ postId: string; commentId: string | null } | null>(null)
  const post = state.detail.data?.post, busy = ['sending', 'uncertain', 'pending'].includes(state.mutation.phase), disabled = busy || !state.available || state.readOnly
  return <section className="event-feed" aria-label="Post thread"><h1>THREAD</h1>
    {!state.available && <p role="status">Thread unavailable for the current account or event.</p>}
    {state.readOnly && <p role="status">This event is read-only.</p>}<ReadStatus slot={state.detail} retry={() => { void controller.loadPost() }} />
    {post && <><article><PostContent post={post} onLink={onLink} renderMedia={renderMedia} /><p>{post.reaction_count} reactions · {post.comment_count} comments</p>
      <button type="button" aria-pressed={post.reacted_by_me} disabled={disabled} onClick={() => { void controller.react() }}>{post.reacted_by_me ? 'Remove reaction' : 'React'}</button>
      <button type="button" onClick={() => onReport({ target_type: 'POST', target_id: post.post_id })}>Report post</button>
      {post.author.user_id === state.userId && <><button type="button" disabled={disabled} onClick={() => { if (controller.beginPostEdit()) onEditPost() }}>Edit post</button><button type="button" disabled={disabled} onClick={() => setDeletion({ postId: post.post_id, commentId: null })}>Delete post</button></>}
    </article>
    <section aria-label="Comments"><h2>Comments</h2><ReadStatus slot={state.comments} retry={() => { void controller.loadPost() }} />
      {state.comments.data?.items.length === 0 && <p>No comments yet.</p>}
      <ol className="feed-list">{state.comments.data?.items.map(comment => <li key={comment.comment_id}><p><strong>{comment.author.display_name}</strong> · <time dateTime={comment.created_at}>{new Date(comment.created_at).toLocaleString()}</time></p><p className="feed-text">{comment.text}</p>
        <button type="button" onClick={() => onReport({ target_type: 'COMMENT', target_id: comment.comment_id })}>Report comment</button>
        {comment.author.user_id === state.userId && <><button type="button" disabled={disabled} onClick={() => { controller.beginCommentEdit(comment.comment_id); document.getElementById('reply-text')?.focus() }}>Edit comment</button><button type="button" disabled={disabled} onClick={() => setDeletion({ postId: post.post_id, commentId: comment.comment_id })}>Delete comment</button></>}
      </li>)}</ol>
      {state.comments.data?.next_cursor && <button type="button" disabled={state.comments.phase === 'loading'} onClick={() => { void controller.moreComments() }}>More comments</button>}
      <form onSubmit={e => { e.preventDefault(); void controller.submitComment() }}><label htmlFor="reply-text">{state.reply.edit ? 'Edit comment text' : 'Comment text'}</label>
        <textarea id="reply-text" value={state.reply.text} disabled={disabled} onChange={e => controller.setReplyText(e.target.value)} aria-describedby="reply-limit" /><p id="reply-limit">{Array.from(state.reply.text).length} / 1000 characters</p>
        <button type="submit" disabled={disabled || !state.reply.text.trim() || Array.from(state.reply.text).length > 1000}>{state.reply.edit ? 'Save comment changes' : 'Send comment'}</button>
        {state.reply.edit && <button type="button" disabled={busy} onClick={controller.cancelCommentEdit}>Discard edit</button>}
      </form>
    </section>
    {deletion?.postId === post.post_id && <fieldset><legend>Confirm deletion</legend><p>This will delete {deletion.commentId ? 'your comment' : 'this post'}.</p><button type="button" disabled={disabled} onClick={() => { if (deletion.commentId) void controller.deleteComment(deletion.commentId); else void controller.deletePost(); setDeletion(null) }}>Confirm deletion</button><button type="button" onClick={() => setDeletion(null)}>Keep content</button></fieldset>}
    </>}
    <WriteStatus controller={controller} />
  </section>
}
