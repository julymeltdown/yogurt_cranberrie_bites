import type { ImageUploadPort } from './media'
import { TransportError, type JsonValue } from '../../shared/api/errors'
import type { DomainOperationId } from '../../shared/api/domain-operations.generated'
import type { PrivacyScope } from '../../shared/query/privacy'
import { blank, createFeedRuntime, failure, type Failure, type FeedOptions, type Slot } from '../event-feed/runtime'
import { decoder, freeze, parse, uuid, type Comment, type Deleted, type Page, type Post, type PostDetail, type Reaction, type Receipt, type ResourceLink } from '../event-feed/contract'

export interface PostDraft { readonly text: string; readonly media_ids: readonly string[]; readonly links: readonly ResourceLink[]; readonly edit: { readonly id: string; readonly revision: number } | null }
interface CommentDraft { readonly text: string; readonly edit: { readonly id: string; readonly revision: number } | null }
type MutationPhase = 'idle' | 'sending' | 'uncertain' | 'pending' | 'failed' | 'succeeded'
export interface MutationState { readonly phase: MutationPhase; readonly key: string | null; readonly operation: string | null; readonly failure: Failure | null; readonly resourceId: string | null }
export interface ThreadState {
  readonly available: boolean; readonly readOnly: boolean; readonly userId: string | null; readonly selectedId: string | null
  readonly detail: Slot<PostDetail>; readonly comments: Slot<Page<Comment>>; readonly draft: PostDraft; readonly reply: CommentDraft; readonly mutation: MutationState
}
interface Command {
  readonly operation: DomainOperationId; readonly suffix: string; readonly method: 'POST' | 'PATCH' | 'DELETE' | 'PUT'
  readonly schema: string; readonly requestSchema: string; readonly body?: JsonValue; readonly query?: Readonly<Record<string, number>>
  readonly resource: string | null; readonly kind: 'post' | 'comment' | 'reaction' | 'delete-post' | 'delete-comment'
  readonly key: string; readonly scope: PrivacyScope
}
export interface ThreadOptions extends FeedOptions { readonly media?: ImageUploadPort }
export function createFeedThreadController(options: ThreadOptions) {
  const emptyDraft = (): PostDraft => ({ text: '', media_ids: [], links: [], edit: null })
  const emptyMutation = (): MutationState => ({ phase: 'idle', key: null, operation: null, failure: null, resourceId: null })
  const initial = (): ThreadState => ({ available: false, readOnly: true, userId: null, selectedId: null, detail: blank(), comments: blank(), draft: emptyDraft(), reply: { text: '', edit: null }, mutation: emptyMutation() })
  let state = freeze(initial()), detailVersion = 0, commentVersion = 0, writeVersion = 0, command: Command | null = null
  const listeners = new Set<() => void>(), cursors = new Set<string>()
  const publish = (patch: Partial<ThreadState>) => { state = freeze({ ...state, ...patch }); listeners.forEach(fn => fn()) }
  const runtime = createFeedRuntime(options, () => {
    detailVersion++; commentVersion++; writeVersion++; command = null; options.media?.cancel(); cursors.clear(); state = freeze(initial()); listeners.forEach(fn => fn())
  }, () => {
    publish({ available: runtime.usable(), readOnly: runtime.access().readOnly, userId: runtime.scope()?.userId ?? null,
      ...(!runtime.usable() ? { detail: blank<PostDetail>(), comments: blank<Page<Comment>>() } : {}) })
  })
  const locked = () => ['sending', 'uncertain', 'pending'].includes(state.mutation.phase)
  const writable = () => runtime.writable() && !locked()
  async function loadPost() {
    if (!runtime.usable() || !state.selectedId) return
    const id = state.selectedId, version = ++detailVersion, scope = runtime.scope()!
    runtime.cancel('comments'); commentVersion++; cursors.clear()
    publish({ detail: { phase: 'loading', data: null, failure: null }, comments: blank() })
    try {
      if (!uuid.test(id)) throw new Error('Invalid post')
      const detail = await runtime.run('detail', s => ({ method: 'GET', path: `/api/concert/events/${s.eventId}/posts/${id}`, access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertPost',
        decode: decoder<PostDetail>('Domain_feed_PostDetail', d => d.post.post_id === id && d.post.event_id === s.eventId && d.comments.every(c => c.post_id === id) && new Set(d.comments.map(c => c.comment_id)).size === d.comments.length) }))
      if (version !== detailVersion || !options.auth.privacy.isCurrent(scope)) return
      publish({ detail: { phase: 'ready', data: detail, failure: null }, comments: { phase: 'ready', data: { items: detail.comments, next_cursor: detail.next_cursor }, failure: null } })
    } catch (error) {
      if (version === detailVersion && options.auth.privacy.isCurrent(scope)) publish({ detail: { phase: 'error', data: null, failure: failure(error) }, comments: blank(), reply: { text: '', edit: null } })
    }
  }
  async function moreComments() {
    const previous = state.comments.data, cursor = previous?.next_cursor, id = state.selectedId
    if (!runtime.usable() || !id || !cursor || !previous || state.comments.phase === 'loading' || state.detail.phase !== 'ready') return
    const version = ++commentVersion, scope = runtime.scope()!
    publish({ comments: { phase: 'loading', data: previous, failure: null } })
    try {
      const page = await runtime.run('comments', s => ({ method: 'GET', path: `/api/concert/events/${s.eventId}/posts/${id}/comments`, access: 'protected', errorFamily: 'domain', domainOperation: 'listConcertComments', query: { cursor, limit: 20 },
        decode: decoder<Page<Comment>>('Domain_feed_CommentList', p => p.items.every(c => c.post_id === id)) }))
      if (version !== commentVersion || !options.auth.privacy.isCurrent(scope)) return
      const items = [...previous.items, ...page.items]
      if (new Set(items.map(c => c.comment_id)).size !== items.length || (page.next_cursor && (page.next_cursor === cursor || cursors.has(page.next_cursor)))) throw new Error('Invalid comments cursor')
      cursors.add(cursor)
      publish({ comments: { phase: 'ready', data: { items, next_cursor: page.next_cursor }, failure: null } })
    } catch (error) {
      if (version === commentVersion && options.auth.privacy.isCurrent(scope)) {
        // Parent authorization must not be bypassed with previously loaded comments.
        publish({ detail: { phase: 'error', data: null, failure: failure(error) }, comments: { phase: 'error', data: null, failure: failure(error) } })
      }
    }
  }
  function checkedReceipt(value: unknown, cmd: Command): Receipt {
    const r = parse<Receipt>('Domain_common_CommandReceipt', value)
    if (r.event_id !== cmd.scope.eventId || r.actor_id !== cmd.scope.userId || r.operation_id !== cmd.operation || r.idempotency_key !== cmd.key
      || (cmd.resource && r.resource_id && r.resource_id !== cmd.resource)
      || Date.parse(r.expires_at) <= Date.parse(r.created_at)
      || (r.state === 'PENDING' && (r.result_status !== null || r.failure_reason !== null))
      || (r.state === 'SUCCEEDED' && (r.result_status !== 200 || !r.resource_id || !r.resource_revision || r.failure_reason !== null))
      || (r.state === 'FAILED' && !r.failure_reason)) throw new Error('Wrong receipt')
    return r
  }
  function complete(cmd: Command, resourceId: string) {
    command = null
    publish({ mutation: { phase: 'succeeded', key: cmd.key, operation: cmd.operation, failure: null, resourceId },
      ...(cmd.kind === 'post' ? { draft: emptyDraft() } : {}), ...(cmd.kind === 'comment' ? { reply: { text: '', edit: null } } : {}) })
    if (cmd.kind === 'delete-post') { detailVersion++; commentVersion++; runtime.cancel('detail'); runtime.cancel('comments'); publish({ detail: { phase: 'error', data: null, failure: 'unavailable' }, comments: blank(), reply: { text: '', edit: null } }) }
    else if (state.selectedId) void loadPost() // reads only; never mutation replay
  }
  function acceptReceipt(cmd: Command, receipt: Receipt) {
    if (receipt.state === 'SUCCEEDED') complete(cmd, receipt.resource_id!)
    else if (receipt.state === 'FAILED') {
      command = null; publish({ mutation: { phase: 'failed', key: cmd.key, operation: cmd.operation, failure: 'failed', resourceId: null } })
    } else if (receipt.state === 'RETIRED') publish({ mutation: { phase: 'uncertain', key: cmd.key, operation: cmd.operation, failure: 'unavailable', resourceId: null } })
    else publish({ mutation: { phase: 'pending', key: cmd.key, operation: cmd.operation, failure: null, resourceId: receipt.resource_id } })
  }
  async function dispatch(input: Omit<Command, 'key' | 'scope'>) {
    if (!writable()) return false
    // Empty/oversized body is rejected before dispatch and before a key is allocated.
    try { parse(input.requestSchema, input.body ?? input.query ?? {}) } catch { publish({ mutation: { ...emptyMutation(), phase: 'failed', failure: 'invalid' } }); return false }
    const cmd = freeze({ ...input, key: crypto.randomUUID(), scope: runtime.scope()! }), version = ++writeVersion
    command = cmd
    publish({ mutation: { phase: 'sending', key: cmd.key, operation: cmd.operation, failure: null, resourceId: null } })
    try {
      const result = await runtime.run('write', s => ({ method: cmd.method, path: `/api/concert/events/${s.eventId}${cmd.suffix}`, access: 'protected', errorFamily: 'domain', domainOperation: cmd.operation, idempotencyKey: cmd.key,
        ...(cmd.query ? { query: cmd.query } : {}), ...(cmd.body !== undefined ? { body: { json: cmd.body } } : {}),
        decode(value, context): { pending: Receipt } | { data: Post | Comment | Reaction | Deleted } {
          if (context.status === 202) { const receipt = checkedReceipt(value, cmd); if (receipt.state !== 'PENDING') throw new Error('Expected pending receipt'); return { pending: receipt } }
          const data = parse<Post | Comment | Reaction | Deleted>(cmd.schema, value)
          const resourceId = 'post_id' in data && !('comment_id' in data) ? data.post_id : 'comment_id' in data ? data.comment_id : 'resource_id' in data ? data.resource_id : null
          if (!resourceId || (cmd.resource && resourceId !== cmd.resource)
            || ('event_id' in data && data.event_id !== s.eventId) || ('comment_id' in data && data.post_id !== state.selectedId)
            || ('author' in data && data.author.user_id !== s.userId)
            || ('reacted_by_me' in data && cmd.kind === 'reaction' && data.reacted_by_me !== (cmd.operation === 'likeConcertPost'))) throw new Error('Wrong mutation context')
          return { data }
        } }))
      if (version !== writeVersion || !options.auth.privacy.isCurrent(cmd.scope)) return false
      if ('pending' in result) acceptReceipt(cmd, result.pending)
      else {
        const d = result.data, resourceId = 'comment_id' in d ? d.comment_id : 'post_id' in d ? d.post_id : d.resource_id
        complete(cmd, resourceId)
      }
      return true
    } catch (error) {
      if (version !== writeVersion || !options.auth.privacy.isCurrent(cmd.scope)) return false
      const uncertain = !(error instanceof TransportError) || error.uncertain
      if (!uncertain) command = null
      publish({ mutation: { phase: uncertain ? 'uncertain' : 'failed', key: cmd.key, operation: cmd.operation, failure: failure(error), resourceId: null } })
      if (!uncertain && (failure(error) === 'unavailable' || failure(error) === 'denied')) publish({ detail: { phase: 'error', data: null, failure: failure(error) }, comments: blank() })
      return false
    }
  }
  return {
    media: options.media,
    read: () => state, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } }, start: runtime.start, dispose: runtime.dispose,
    loadPost, moreComments,
    select(id: string | null) {
      if (locked()) return false
      detailVersion++; commentVersion++; runtime.cancel('detail'); runtime.cancel('comments'); cursors.clear()
      publish({ selectedId: id, detail: blank(), comments: blank(), reply: { text: '', edit: null }, ...(state.draft.edit ? { draft: emptyDraft() } : {}), mutation: emptyMutation() })
      if (id) void loadPost(); return true
    },
    setPostText(text: string) { if (writable()) publish({ draft: { ...state.draft, text } }) },
    setReplyText(text: string) { if (writable()) publish({ reply: { ...state.reply, text } }) },
    beginPostEdit() {
      const post = state.detail.data?.post
      if (!writable() || !post || post.author.user_id !== state.userId) return false
      publish({ draft: { text: post.text, media_ids: post.media_ids, links: post.links, edit: { id: post.post_id, revision: post.revision } } }); return true
    },
    beginCommentEdit(id: string) {
      const comment = state.comments.data?.items.find(c => c.comment_id === id)
      if (!writable() || !comment || comment.author.user_id !== state.userId) return false
      publish({ reply: { text: comment.text, edit: { id, revision: comment.revision } } }); return true
    },
    cancelPostEdit() { if (!locked()) publish({ draft: emptyDraft() }) },
    cancelCommentEdit() { if (!locked()) publish({ reply: { text: '', edit: null } }) },
    attachReadyMedia() {
      if (!writable() || !options.media) return false
      const upload = options.media.read(), scope = runtime.scope()!
      if (upload.phase !== 'ready' || !upload.attachment || !upload.scope || upload.scope.userId !== scope.userId || upload.scope.eventId !== scope.eventId || upload.scope.generation !== scope.generation) return false
      try { parse('Domain_feed_Media', upload.attachment) } catch { return false }
      const media = upload.attachment
      if (media.purpose !== 'POST' || media.read_path !== `/api/concert/events/${scope.eventId}/media/${media.media_id}/content` || state.draft.media_ids.includes(media.media_id) || state.draft.media_ids.length >= 4) return false
      publish({ draft: { ...state.draft, media_ids: [...state.draft.media_ids, media.media_id] } }); options.media.cancel(); return true
    },
    removeMedia(id: string) { if (writable()) publish({ draft: { ...state.draft, media_ids: state.draft.media_ids.filter(value => value !== id) } }) },
    submitPost() {
      const d = state.draft, edit = d.edit
      if (!d.text.trim() || (edit && (state.detail.data?.post.post_id !== edit.id || state.detail.data.post.revision !== edit.revision || state.detail.data.post.author.user_id !== state.userId))) return Promise.resolve(false)
      return dispatch({ operation: edit ? 'editConcertPost' : 'createConcertPost', suffix: edit ? `/posts/${edit.id}` : '/posts', method: edit ? 'PATCH' : 'POST', schema: 'Domain_feed_PostPublic', requestSchema: edit ? 'Domain_feed_EditPost' : 'Domain_feed_CreatePost',
        body: { text: d.text, media_ids: d.media_ids, links: d.links.map(l => ({ ...l })), ...(edit ? { expected_revision: edit.revision } : {}) }, resource: edit?.id ?? null, kind: 'post' })
    },
    submitComment() {
      const post = state.detail.data?.post, d = state.reply
      if (!post || !d.text.trim()) return Promise.resolve(false)
      if (d.edit && !state.comments.data?.items.some(c => c.comment_id === d.edit!.id && c.revision === d.edit!.revision && c.author.user_id === state.userId)) return Promise.resolve(false)
      return dispatch({ operation: d.edit ? 'editConcertComment' : 'createConcertComment', suffix: d.edit ? `/comments/${d.edit.id}` : `/posts/${post.post_id}/comments`, method: d.edit ? 'PATCH' : 'POST', schema: 'Domain_feed_CommentPublic', requestSchema: d.edit ? 'Domain_feed_EditComment' : 'Domain_feed_CreateComment',
        body: { text: d.text, ...(d.edit ? { expected_revision: d.edit.revision } : { post_revision: post.revision }) }, resource: d.edit?.id ?? null, kind: 'comment' })
    },
    react() {
      const post = state.detail.data?.post
      if (!post) return Promise.resolve(false)
      return dispatch({ operation: post.reacted_by_me ? 'unlikeConcertPost' : 'likeConcertPost', suffix: `/posts/${post.post_id}/reaction`, method: post.reacted_by_me ? 'DELETE' : 'PUT', schema: 'Domain_feed_ReactionState', requestSchema: 'Domain_common_EmptyRequest', resource: post.post_id, kind: 'reaction' })
    },
    deletePost() {
      const post = state.detail.data?.post
      if (!post || post.author.user_id !== state.userId) return Promise.resolve(false)
      return dispatch({ operation: 'deleteConcertPost', suffix: `/posts/${post.post_id}`, method: 'DELETE', schema: 'Domain_common_Deleted', requestSchema: 'Domain_common_RevisionRequest', query: { expected_revision: post.revision }, resource: post.post_id, kind: 'delete-post' })
    },
    deleteComment(id: string) {
      const comment = state.comments.data?.items.find(c => c.comment_id === id)
      if (!comment || comment.author.user_id !== state.userId) return Promise.resolve(false)
      return dispatch({ operation: 'deleteConcertComment', suffix: `/comments/${id}`, method: 'DELETE', schema: 'Domain_common_Deleted', requestSchema: 'Domain_common_RevisionRequest', query: { expected_revision: comment.revision }, resource: id, kind: 'delete-comment' })
    },
    async checkStatus() {
      const cmd = command
      if (!cmd || !['uncertain', 'pending'].includes(state.mutation.phase) || !runtime.usable()) return false
      const version = ++writeVersion
      publish({ mutation: { ...state.mutation, phase: 'pending', failure: null } })
      try {
        const receipt = await runtime.run('status', s => ({ method: 'GET', path: `/api/concert/events/${s.eventId}/commands`, access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertCommandStatus', query: { operation_id: cmd.operation, idempotency_key: cmd.key }, decode: value => checkedReceipt(value, cmd) }))
        if (version === writeVersion && options.auth.privacy.isCurrent(cmd.scope)) acceptReceipt(cmd, receipt)
        return true
      } catch (error) {
        if (version === writeVersion && options.auth.privacy.isCurrent(cmd.scope)) publish({ mutation: { ...state.mutation, phase: 'uncertain', failure: failure(error) } })
        return false
      }
    },
  }
}
export type FeedThreadController = ReturnType<typeof createFeedThreadController>
