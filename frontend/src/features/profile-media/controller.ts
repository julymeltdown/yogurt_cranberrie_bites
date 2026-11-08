/**
 * WP-06 T-506/T-510: the profile media composer controller.
 *
 * The composer is a small state machine with one invariant: **the draft is never
 * lost**. A failed upload keeps the description, the visibility and every alt
 * text exactly as they were, so the user retries instead of retyping. A
 * cancelled upload clears the pending attachment but keeps the text.
 *
 * Publishing is deliberately independent of everything else:
 *
 * * it does not read or write an intro post;
 * * it does not read or write the room;
 * * it does not convert pixels — the uploaded bytes are the photograph.
 *
 * The controller never decides visibility. It sends the chosen `visibility` and
 * renders what the server returns.
 */
import type { HttpClient } from '../../shared/api/http'
import type { PrivacyScope } from '../../shared/query/privacy'
import {
  createProfilePostBody,
  decodeProfilePost,
  PATHS,
  type CreateProfilePost,
  type ProfilePost,
} from './contract'
import {
  createProfileMediaUpload,
  cryptoIdempotencyKey,
  failureMessage,
  type ProfileMediaUpload,
  type UploadAuth,
  type UploadState,
} from './upload'
import {
  draftIsPublishable,
  validateDraft,
  type FieldIssue,
  type PostVisibility,
  type ProfilePostDraft,
} from './validation'

/** One picked image, before it is uploaded. */
export type ComposerAttachment = Readonly<{
  localId: string
  fileName: string
  contentType: string
  size: number
  width: number
  height: number
  altText: string
  /** The upload for this attachment, once started. */
  upload: ProfileMediaUpload | null
}>

export type PublishPhase = 'idle' | 'publishing' | 'published' | 'failed'

export type ComposerState = Readonly<{
  draft: Readonly<{
    description: string
    visibility: PostVisibility
  }>
  attachments: readonly ComposerAttachment[]
  issues: readonly FieldIssue[]
  phase: PublishPhase
  /** The published post, once the server confirmed it. */
  published: ProfilePost | null
  /** A line the composer shows for the current outcome. */
  notice: string | null
  /** Whether the notice is an error, so the UI can mark it. */
  noticeIsError: boolean
}>

export type ComposerDeps = Readonly<{
  auth: UploadAuth
  scope: PrivacyScope
  digest: (bytes: Uint8Array) => Promise<string>
  nextLocalId: () => string
  /** The idempotency key source. Injected so a test can pin it. */
  idempotencyKey?: () => string
}>

const EMPTY_DRAFT = Object.freeze({ description: '', visibility: 'PUBLIC' as PostVisibility })

/**
 * The composer controller.
 *
 * `subscribe`/`read` match `useSyncExternalStore`, so the component re-renders on
 * every state change and nothing else.
 */
export function createProfileMediaComposer(deps: ComposerDeps) {
  const { auth, scope, digest, nextLocalId } = deps
  const key = deps.idempotencyKey ?? cryptoIdempotencyKey
  let state: ComposerState = Object.freeze({
    draft: EMPTY_DRAFT,
    attachments: Object.freeze([]),
    issues: Object.freeze([]),
    phase: 'idle',
    published: null,
    notice: null,
    noticeIsError: false,
  })
  const listeners = new Set<() => void>()
  const lifetime = new AbortController()
  let disposed = false

  const notify = () => {
    for (const listener of Array.from(listeners)) listener()
  }
  const publish = (next: Partial<ComposerState>) => {
    state = Object.freeze({ ...state, ...next })
    notify()
  }
  const revalidate = (draft: ProfilePostDraft) => validateDraft(draft)

  /** The draft as the validator sees it. */
  const asDraft = (source: ComposerState): ProfilePostDraft => Object.freeze({
    description: source.draft.description,
    visibility: source.draft.visibility,
    attachments: source.attachments.map(attachment => Object.freeze({
      localId: attachment.localId,
      altText: attachment.altText,
      contentType: attachment.contentType as ProfilePostDraft['attachments'][number]['contentType'],
      size: attachment.size,
      width: attachment.width,
      height: attachment.height,
    })),
  })

  function edit(next: { description?: string; visibility?: PostVisibility }) {
    const draft = Object.freeze({ ...state.draft, ...next })
    const candidate = Object.freeze({ ...state, draft })
    publish({ draft, issues: revalidate(asDraft(candidate)) })
  }

  /**
   * Add one picked image.
   *
   * The count and file limits are checked here, so an unsupported file is
   * refused before any upload exists. The returned issue is also in `issues`.
   */
  function addAttachment(input: Omit<ComposerAttachment, 'upload'>): FieldIssue | null {
    const candidate = Object.freeze({
      ...state,
      attachments: Object.freeze([...state.attachments, Object.freeze({ ...input, upload: null })]),
    })
    const issues = revalidate(asDraft(candidate))
    // A count issue is about the list, not about the new attachment, so it is
    // checked first: the fifth image is refused before its own fields matter.
    const mine = issues.find(issue => issue.code === 'TOO_MANY')
      ?? issues.find(issue => issue.field !== 'description' && issue.field !== 'visibility'
        && issue.attachment === candidate.attachments.length - 1)
    if (mine) {
      // A refused pick does not enter the list at all.
      publish({ issues })
      return mine
    }
    publish({ attachments: candidate.attachments, issues })
    return null
  }

  function removeAttachment(localId: string) {
    const attachment = state.attachments.find(value => value.localId === localId)
    // Removing an attachment cancels its upload first, so a removed image never
    // becomes a published asset.
    attachment?.upload?.cancel()
    const candidate = Object.freeze({
      ...state,
      attachments: Object.freeze(state.attachments.filter(value => value.localId !== localId)),
    })
    publish({ attachments: candidate.attachments, issues: revalidate(asDraft(candidate)) })
  }

  function setAltText(localId: string, altText: string) {
    const candidate = Object.freeze({
      ...state,
      attachments: Object.freeze(state.attachments.map(attachment =>
        attachment.localId === localId ? Object.freeze({ ...attachment, altText }) : attachment)),
    })
    publish({ attachments: candidate.attachments, issues: revalidate(asDraft(candidate)) })
  }

  /** The upload state of every attachment, for the UI. */
  function uploadStates(): readonly (UploadState | null)[] {
    return state.attachments.map(attachment => attachment.upload?.read() ?? null)
  }

  /**
   * Publish: upload every attachment, then create the post.
   *
   * The draft is captured before the first request and never mutated by this
   * function, so a failure leaves the user's text exactly as it was.
   */
  async function publishPost(): Promise<ProfilePost | null> {
    if (disposed) return null
    const draft = asDraft(state)
    const issues = revalidate(draft)
    if (issues.length > 0) {
      publish({ issues, phase: 'failed', notice: issues[0]!.message, noticeIsError: true })
      return null
    }
    if (state.phase === 'publishing') return null
    publish({ phase: 'publishing', notice: 'Uploading your images…', noticeIsError: false })
    const mediaIds: string[] = []
    // Upload in order so the post's attachment positions match the composer's.
    for (const attachment of state.attachments) {
      const upload = attachment.upload ?? createProfileMediaUpload({
        auth,
        scope,
        digest,
        nextLocalId,
        idempotencyKey: key,
      })
      const replaced = Object.freeze({
        ...state,
        attachments: Object.freeze(state.attachments.map(value =>
          value.localId === attachment.localId ? Object.freeze({ ...value, upload }) : value)),
      })
      state = replaced
      notify()
      const bytes = await readBytes(attachment)
      const result = await upload.start(bytes, attachment.contentType)
      if (result.phase === 'cancelled') {
        publish({ phase: 'idle', notice: 'Upload cancelled. Your draft is kept.', noticeIsError: true })
        return null
      }
      if (result.phase !== 'ready' || !result.media) {
        const failure = result.failure ?? { kind: 'unknown' as const, uncertain: false }
        publish({
          phase: 'failed',
          notice: `${failureMessage(failure)} Your draft is kept.`,
          noticeIsError: true,
        })
        return null
      }
      mediaIds.push(result.media.media_id)
    }
    const body: CreateProfilePost = {
      description: draft.description,
      media_ids: mediaIds,
      visibility: draft.visibility,
      expires_at: null,
    }
    try {
      const created = await auth.privacy.run(scope, ({ signal }) => auth.request({
        path: PATHS.createPost,
        method: 'POST',
        access: 'protected',
        errorFamily: 'domain',
        domainOperation: 'createProfilePost',
        idempotencyKey: key(),
        signal: combine(signal, lifetime.signal),
        body: { json: createProfilePostBody(body) },
        decode: decodeProfilePost,
      }), lifetime.signal)
      publish({
        phase: 'published',
        published: created.data,
        notice: 'Posted.',
        noticeIsError: false,
      })
      return created.data
    } catch {
      // The draft is untouched: a failed publish keeps the description, the
      // visibility and every alt text so the user can retry.
      publish({
        phase: 'failed',
        notice: 'Your post was not published. Your draft is kept; try again.',
        noticeIsError: true,
      })
      return null
    }
  }

  /**
   * Cancel everything in flight.
   *
   * Called when the composer unmounts or the user leaves the screen. Every
   * in-flight upload is aborted and marked cancelled, so nothing is published
   * after the screen is gone and nothing is left to retry.
   */
  function cancelAll() {
    for (const attachment of state.attachments) attachment.upload?.cancel()
    if (state.phase === 'publishing') {
      publish({ phase: 'idle', notice: 'Upload cancelled. Your draft is kept.', noticeIsError: true })
    }
    notify()
  }

  function dispose() {
    if (disposed) return
    disposed = true
    cancelAll()
    lifetime.abort()
    listeners.clear()
  }

  /**
   * Read the bytes of one attachment.
   *
   * The composer holds no file handle after the pick: the caller attaches a
   * `read` function on the attachment's source. A test supplies the bytes
   * directly; the browser implementation reads the picked `File`.
   */
  const sources = new Map<string, () => Promise<Uint8Array>>()
  function provideSource(localId: string, read: () => Promise<Uint8Array>) {
    sources.set(localId, read)
  }
  async function readBytes(attachment: ComposerAttachment): Promise<Uint8Array> {
    const read = sources.get(attachment.localId)
    if (!read) throw new Error('The image is no longer available. Pick it again.')
    return read()
  }

  return Object.freeze({
    read: () => state,
    subscribe(listener: () => void) {
      listeners.add(listener)
      return () => { listeners.delete(listener) }
    },
    edit,
    addAttachment,
    removeAttachment,
    setAltText,
    provideSource,
    uploadStates,
    publishPost,
    cancelAll,
    dispose,
    publishable: () => draftIsPublishable(asDraft(state)),
  })
}

export type ProfileMediaComposerController = ReturnType<typeof createProfileMediaComposer>

/** Combine two abort signals without leaking a listener. */
function combine(a: AbortSignal, b: AbortSignal): AbortSignal {
  if (typeof AbortSignal.any === 'function') return AbortSignal.any([a, b])
  const controller = new AbortController()
  const abort = () => controller.abort()
  if (a.aborted || b.aborted) controller.abort()
  else {
    a.addEventListener('abort', abort, { once: true })
    b.addEventListener('abort', abort, { once: true })
  }
  return controller.signal
}

/** The request surface, re-exported so the component does not import the client. */
export type ComposerRequest = HttpClient['request']
