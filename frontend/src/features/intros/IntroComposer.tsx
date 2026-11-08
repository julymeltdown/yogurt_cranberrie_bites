import { useId, useReducer, useSyncExternalStore } from 'react'
import {
  canReview,
  draftAudience,
  initial,
  reduce,
  saveBody,
  stepIndex,
  validate,
  type ComposerState,
  type Step,
} from './composer-reducer'
import { IntroAudienceFields } from './IntroAudienceFields'
import { MAX_BODY, MAX_SINGLE_LINE } from './contract'
import type { IntroComposerController } from './controller'
import './intros.css'

/**
 * WP-05 T-408: the intro composer screen.
 *
 * Three steps: audience, content, review. The review step shows the
 * **server-derived** `audience_summary` — the string the server returned from
 * the save — and never computes one from the bounds. If the draft has not been
 * saved yet, the review step says so instead of inventing a summary.
 *
 * Publish and save-draft are distinct actions with distinct outcomes: saving
 * keeps the post a draft, publishing makes it visible to its audience. Publishing
 * without a body is blocked here and rejected by the server as well.
 */
export function IntroComposer({ controller }: { readonly controller: IntroComposerController }) {
  const [state, dispatch] = useReducer(reduce, undefined, () => initial([]))
  const id = useId()
  const remote = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const validation = validate(state.draft, true)
  const index = stepIndex(state.step)
  return (
    <section className="intro-composer" aria-labelledby={`${id}-title`}>
      <header>
        <p>MUTUALS-INTRO POST</p>
        <h1 id={`${id}-title`}>Write an introduction</h1>
      </header>
      <ol className="intro-composer-steps" aria-label="Composer steps">
        {(['audience', 'content', 'review'] as const).map((step, position) => (
          <li key={step} aria-current={state.step === step ? 'step' : undefined}>
            {position + 1}. {STEP_LABEL[step]}
          </li>
        ))}
      </ol>
      {state.step === 'audience' && (
        <IntroAudienceFields
          gender={state.draft.gender}
          ageEnabled={state.draft.age.enabled}
          minimum={state.draft.age.minimum}
          maximum={state.draft.age.maximum}
          error={validation.audience}
          onGender={(value) => dispatch({ type: 'gender', value })}
          onAgeEnabled={(value) => dispatch({ type: 'ageEnabled', value })}
          onMinimum={(value) => dispatch({ type: 'ageMinimum', value })}
          onMaximum={(value) => dispatch({ type: 'ageMaximum', value })}
        />
      )}
      {state.step === 'content' && (
        <ContentStep id={id} state={state} dispatch={dispatch} error={validation.content} />
      )}
      {state.step === 'review' && (
        <ReviewStep
          state={state}
          publishing={remote.publishing}
          message={state.message || remote.message}
        />
      )}
      <div className="intro-composer-actions">
        <button type="button" disabled={index === 0 || remote.busy} onClick={() => dispatch({ type: 'back' })}>
          Back
        </button>
        {state.step !== 'review' ? (
          <button
            type="button"
            disabled={!canReview(state) || remote.busy}
            onClick={() => dispatch({ type: 'next' })}
          >
            Continue
          </button>
        ) : (
          <>
            <button
              type="button"
              disabled={remote.busy || validation.audience !== null || validation.content !== null}
              onClick={() => void controller.saveDraft(saveBody(state), (saved) =>
                dispatch({ type: 'saved', introId: saved.introId, revision: saved.revision, audienceSummary: saved.audienceSummary }),
              )}
            >
              Save draft
            </button>
            <button
              type="button"
              disabled={
                remote.busy ||
                state.introId === null ||
                !validation.valid ||
                state.draft.body.trim().length === 0
              }
              onClick={() => void controller.publish(state.introId!, state.revision, (published) =>
                dispatch({ type: 'saved', introId: published.introId, revision: published.revision, audienceSummary: published.audienceSummary }),
              )}
            >
              Publish
            </button>
          </>
        )}
      </div>
    </section>
  )
}

const STEP_LABEL: Readonly<Record<Step, string>> = Object.freeze({
  audience: 'Who can see it',
  content: 'Your introduction',
  review: 'Review',
})

function ContentStep({
  id,
  state,
  dispatch,
  error,
}: {
  readonly id: string
  readonly state: ComposerState
  readonly dispatch: (action: Parameters<typeof reduce>[1]) => void
  readonly error: string | null
}) {
  const bodyLength = [...state.draft.body].length
  return (
    <div className="intro-composer-content">
      <label htmlFor={`${id}-body`}>Your introduction</label>
      <textarea
        id={`${id}-body`}
        value={state.draft.body}
        aria-invalid={error !== null}
        onChange={(event) => dispatch({ type: 'body', value: event.target.value })}
      />
      <p className="intro-composer-count">
        {bodyLength} / {MAX_BODY} Unicode code points.
      </p>
      {error !== null && (
        <p className="intro-composer-error" role="alert">
          {error}
        </p>
      )}
      <label htmlFor={`${id}-seeking`}>What you are looking for (optional)</label>
      <input
        id={`${id}-seeking`}
        type="text"
        maxLength={MAX_SINGLE_LINE}
        value={state.draft.currentlySeeking}
        onChange={(event) => dispatch({ type: 'currentlySeeking', value: event.target.value })}
      />
      <label htmlFor={`${id}-style`}>How you like to talk (optional)</label>
      <input
        id={`${id}-style`}
        type="text"
        maxLength={MAX_SINGLE_LINE}
        value={state.draft.interactionStyle}
        onChange={(event) => dispatch({ type: 'interactionStyle', value: event.target.value })}
      />
    </div>
  )
}

function ReviewStep({
  state,
  publishing,
  message,
}: {
  readonly state: ComposerState
  readonly publishing: boolean
  readonly message: string
}) {
  const audience = draftAudience(state.draft)
  return (
    <div className="intro-composer-review">
      <h2>Review</h2>
      <dl>
        <dt>Who can see it</dt>
        <dd>
          {/*
            The server-derived summary, verbatim. When the draft has not been
            saved yet there is no summary to show, and the composer says so
            rather than computing one locally.
          */}
          {state.audienceSummary ?? 'Save the draft to see the audience summary.'}
        </dd>
        <dt>Your introduction</dt>
        <dd>{state.draft.body.trim().length === 0 ? 'Nothing written yet.' : state.draft.body}</dd>
        <dt>Photographs</dt>
        <dd>{state.draft.attachmentMediaIds.length}</dd>
      </dl>
      {audience === null && (
        <p className="intro-composer-error" role="alert">
          The audience is incomplete. Go back and fix it before saving.
        </p>
      )}
      {message !== '' && (
        <p className="intro-composer-error" role="alert">
          {message}
        </p>
      )}
      {publishing && <p role="status">Saving…</p>}
      <p className="intro-composer-note">
        Publishing makes this post visible to the audience above. Saving a draft keeps it private to you.
      </p>
    </div>
  )
}
