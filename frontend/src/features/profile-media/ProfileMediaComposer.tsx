/**
 * WP-06 T-506: the profile media composer.
 *
 * A small screen with one job: pick up to four images, write an optional
 * description and an optional alt text per image, choose who can see the post,
 * and publish.
 *
 * What it deliberately does **not** offer:
 *
 * * no poster import and no pixel conversion — an ordinary photograph is stored
 *   as the photograph (product-plan.md §2 hard rule 5);
 * * no intro post and no room step — publishing here touches neither;
 * * no audience, age or gender condition — a profile post is public or private.
 *
 * Accessibility: every control is a labelled native control, the character
 * counters are `aria-describedby` on their fields, an over-limit field is
 * `aria-invalid` with a `role="alert"` message, and the outcome line is a live
 * region. The gallery of picked images is a list, not a grid of buttons, so a
 * keyboard user tabs through the fields in reading order.
 */
import { useEffect, useId, useSyncExternalStore } from 'react'
import { codePointLength } from '../../shared/validation/text'
import type { ProfileMediaComposerController } from './controller'
import { failureMessage } from './upload'
import {
  ACCEPTED_CONTENT_TYPES,
  MAX_ALT_TEXT_CODE_POINTS,
  MAX_ATTACHMENTS,
  MAX_DESCRIPTION_CODE_POINTS,
  remaining,
} from './validation'
import './profile-media.css'

/** The props the screen passes in. */
export type ProfileMediaComposerProps = Readonly<{
  composer: ProfileMediaComposerController
  /** Read a picked `File` into bytes. Injected so tests do not need a real File. */
  readFile?: (file: File) => Promise<Uint8Array>
  /** Read the pixel dimensions of a picked file. */
  measure?: (file: File) => Promise<{ width: number; height: number }>
  /** Called after a successful publish, so the gallery can refresh. */
  onPublished?: () => void
}>

export function ProfileMediaComposer({ composer, readFile, measure, onPublished }: ProfileMediaComposerProps) {
  const state = useSyncExternalStore(composer.subscribe, composer.read, composer.read)
  const id = useId()
  // Leaving the screen mid-upload cancels cleanly: every in-flight upload is
  // aborted and nothing is published after unmount.
  useEffect(() => () => { composer.cancelAll() }, [composer])
  const descriptionLength = codePointLength(state.draft.description)
  const descriptionOver = descriptionLength > MAX_DESCRIPTION_CODE_POINTS
  const busy = state.phase === 'publishing'
  const issues = state.issues
  const descriptionIssue = issues.find(issue => issue.field === 'description')

  async function pick(files: FileList | null) {
    if (!files) return
    for (const file of Array.from(files)) {
      if (state.attachments.length >= MAX_ATTACHMENTS) break
      const dimensions = measure ? await measure(file) : { width: 1, height: 1 }
      const localId = `${file.name}-${file.size}-${state.attachments.length}-${Date.now()}`
      const issue = composer.addAttachment({
        localId,
        fileName: file.name,
        contentType: file.type,
        size: file.size,
        width: dimensions.width,
        height: dimensions.height,
        altText: '',
      })
      if (issue) continue
      if (readFile) composer.provideSource(localId, () => readFile(file))
    }
  }

  return <section className="profile-media-composer" aria-labelledby={`${id}-title`}>
    <h1 id={`${id}-title`}>New post</h1>
    <p className="profile-media-hint">
      Share a photograph or your own artwork. It appears on your profile, not in your introduction or your room.
    </p>

    <form onSubmit={event => {
      event.preventDefault()
      void composer.publishPost().then(post => { if (post) onPublished?.() })
    }}>
      <fieldset disabled={busy}>
        <legend>Images</legend>
        <label htmlFor={`${id}-files`}>Add images</label>
        <input
          id={`${id}-files`}
          type="file"
          accept={ACCEPTED_CONTENT_TYPES.join(',')}
          multiple
          aria-describedby={`${id}-files-help`}
          onChange={event => { void pick(event.target.files) }}
        />
        <p id={`${id}-files-help`}>
          Up to {MAX_ATTACHMENTS} JPEG, PNG or WebP images, 10 MiB each. GIF and video are not supported.
          {state.attachments.length} of {MAX_ATTACHMENTS} chosen.
        </p>
        {state.attachments.length > 0 && <ul className="profile-media-attachments">
          {state.attachments.map((attachment, index) => {
            const altIssue = issues.find(issue => issue.field === 'alt_text' && issue.attachment === index)
            const upload = attachment.upload?.read() ?? null
            return <li key={attachment.localId}>
              <span className="profile-media-file">{attachment.fileName}</span>
              <label htmlFor={`${id}-alt-${index}`}>Alt text for image {index + 1}</label>
              <textarea
                id={`${id}-alt-${index}`}
                value={attachment.altText}
                aria-describedby={`${id}-alt-count-${index}`}
                aria-invalid={altIssue !== undefined}
                onChange={event => composer.setAltText(attachment.localId, event.target.value)}
              />
              <p id={`${id}-alt-count-${index}`} role={altIssue ? 'alert' : undefined}>
                {codePointLength(attachment.altText)} / {MAX_ALT_TEXT_CODE_POINTS} characters.
                {altIssue && ` ${altIssue.message}`}
              </p>
              {upload && upload.phase !== 'idle' && <p role="status">
                {upload.phase === 'ready' ? 'Image ready.' : upload.phase === 'failed'
                  ? `${failureMessage(upload.failure ?? { kind: 'unknown', uncertain: false })}`
                  : upload.phase === 'cancelled' ? 'Upload cancelled.' : 'Uploading…'}
              </p>}
              <button type="button" onClick={() => composer.removeAttachment(attachment.localId)}>
                Remove image {index + 1}
              </button>
            </li>
          })}
        </ul>}
      </fieldset>

      <label htmlFor={`${id}-description`}>Description</label>
      <textarea
        id={`${id}-description`}
        value={state.draft.description}
        aria-describedby={`${id}-description-count`}
        aria-invalid={descriptionOver || descriptionIssue !== undefined}
        disabled={busy}
        onChange={event => composer.edit({ description: event.target.value })}
      />
      <p id={`${id}-description-count`} role={descriptionOver ? 'alert' : undefined}>
        {descriptionLength} / {MAX_DESCRIPTION_CODE_POINTS} characters.
        {descriptionOver && ' Shorten the description before publishing.'}
        {!descriptionOver && ` ${remaining(state.draft.description, MAX_DESCRIPTION_CODE_POINTS)} left.`}
      </p>

      <fieldset disabled={busy}>
        <legend>Who can see this post</legend>
        <label>
          <input
            type="radio"
            name={`${id}-visibility`}
            value="PUBLIC"
            checked={state.draft.visibility === 'PUBLIC'}
            onChange={() => composer.edit({ visibility: 'PUBLIC' })}
          />
          Anyone can see it on your profile
        </label>
        <label>
          <input
            type="radio"
            name={`${id}-visibility`}
            value="PRIVATE"
            checked={state.draft.visibility === 'PRIVATE'}
            onChange={() => composer.edit({ visibility: 'PRIVATE' })}
          />
          Only you can see it
        </label>
      </fieldset>

      <div className="profile-media-actions">
        <button type="submit" disabled={busy || issues.length > 0 || !composer.publishable()}>
          {busy ? 'Publishing…' : 'Publish post'}
        </button>
        {busy && <button type="button" onClick={() => composer.cancelAll()}>Cancel upload</button>}
      </div>
    </form>

    <p role="status" aria-live="polite" className={state.noticeIsError ? 'profile-media-error' : undefined}>
      {state.notice ?? ''}
    </p>
    {state.phase === 'failed' && <p className="profile-media-hint">
      Your draft is kept. Fix the highlighted field or try publishing again.
    </p>}
  </section>
}
