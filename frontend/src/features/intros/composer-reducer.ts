import {
  ANYONE,
  audienceFromInclusive,
  MAX_BODY,
  MAX_SINGLE_LINE,
  type Audience,
  type GenderCondition,
} from './contract'

/**
 * WP-05 T-406: the intro composer reducer.
 *
 * Three steps, in this order: **audience → content → review**. The order is the
 * product's: a user decides who may see the post before writing it, so the
 * composer can explain the consequence while they write rather than after.
 *
 * Two rules shape the design:
 *
 * * **Draft state survives step navigation.** Every field lives in one draft
 *   object that moving between steps never touches. Going back and forward is a
 *   step change, not a reset.
 * * **Validation is derived, not stored.** There is no `valid` flag in the
 *   state. [`validate`] computes the answer from the draft every time it is
 *   asked, so a stale flag cannot disagree with the fields it describes.
 *
 * The reducer is pure: `reduce(state, action)` returns a new state and never
 * touches the network, the clock or storage.
 */

export const STEPS = ['audience', 'content', 'review'] as const
export type Step = (typeof STEPS)[number]

/** The inclusive age inputs as the interface holds them, before conversion. */
export interface AgeInput {
  readonly enabled: boolean
  /** The inclusive minimum, as typed. */
  readonly minimum: string
  /** The inclusive maximum, as typed. */
  readonly maximum: string
}

export interface ComposerDraft {
  readonly gender: GenderCondition
  readonly age: AgeInput
  readonly body: string
  readonly currentlySeeking: string
  readonly interactionStyle: string
  /** The favorites snapshot to send. Taken from the profile when the composer opens. */
  readonly favoritesSnapshot: readonly string[]
  /** The uploaded attachment ids, in the author's order. */
  readonly attachmentMediaIds: readonly string[]
  readonly expiresAt: string | null
}

export interface ComposerState {
  readonly step: Step
  readonly draft: ComposerDraft
  /** The server-derived summary, once the draft has been saved. */
  readonly audienceSummary: string | null
  /** Set when a save or publish failed; cleared by the next edit. */
  readonly message: string
  /** The stored revision, once the draft exists on the server. */
  readonly revision: number
  readonly introId: string | null
}

export type Action =
  | { readonly type: 'go'; readonly step: Step }
  | { readonly type: 'back' }
  | { readonly type: 'next' }
  | { readonly type: 'gender'; readonly value: GenderCondition }
  | { readonly type: 'ageEnabled'; readonly value: boolean }
  | { readonly type: 'ageMinimum'; readonly value: string }
  | { readonly type: 'ageMaximum'; readonly value: string }
  | { readonly type: 'body'; readonly value: string }
  | { readonly type: 'currentlySeeking'; readonly value: string }
  | { readonly type: 'interactionStyle'; readonly value: string }
  | { readonly type: 'attachments'; readonly value: readonly string[] }
  | { readonly type: 'expiresAt'; readonly value: string | null }
  | { readonly type: 'saved'; readonly introId: string; readonly revision: number; readonly audienceSummary: string }
  | { readonly type: 'message'; readonly value: string }
  | { readonly type: 'reset'; readonly favoritesSnapshot: readonly string[] }

export function emptyDraft(favoritesSnapshot: readonly string[] = []): ComposerDraft {
  return Object.freeze({
    gender: 'any' as GenderCondition,
    age: Object.freeze({ enabled: false, minimum: '', maximum: '' }),
    body: '',
    currentlySeeking: '',
    interactionStyle: '',
    favoritesSnapshot: Object.freeze([...favoritesSnapshot]),
    attachmentMediaIds: Object.freeze([]),
    expiresAt: null,
  })
}

export function initial(favoritesSnapshot: readonly string[] = []): ComposerState {
  return Object.freeze({
    step: 'audience' as Step,
    draft: emptyDraft(favoritesSnapshot),
    audienceSummary: null,
    message: '',
    revision: 0,
    introId: null,
  })
}

/** The audience value the draft currently describes, or `null` when invalid. */
export function draftAudience(draft: ComposerDraft): Audience | null {
  if (!draft.age.enabled) return audienceFromInclusive(draft.gender, null)
  const minimum = parseAge(draft.age.minimum)
  const maximum = parseAge(draft.age.maximum)
  if (minimum === null || maximum === null) return null
  return audienceFromInclusive(draft.gender, { minimum, maximumInclusive: maximum })
}

function parseAge(value: string): number | null {
  // Strict decimal only: `18.0`, `+18` and ` 18` are not ages.
  if (!/^\d{1,3}$/.test(value.trim())) return null
  return Number.parseInt(value.trim(), 10)
}

/** The validation result, computed from the draft. */
export interface Validation {
  readonly audience: string | null
  readonly content: string | null
  readonly review: string | null
  readonly valid: boolean
}

/**
 * Validate the draft. This is the only place validity is decided, so a
 * component asking "may I continue?" and the review step asking "may I publish?"
 * always get the same answer.
 */
export function validate(draft: ComposerDraft, publishing: boolean): Validation {
  let audience: string | null = null
  if (draft.age.enabled) {
    const minimum = parseAge(draft.age.minimum)
    const maximum = parseAge(draft.age.maximum)
    if (minimum === null || maximum === null) {
      audience = 'Enter both age limits as whole numbers.'
    } else if (draftAudience(draft) === null) {
      audience =
        minimum > maximum
          ? 'The youngest age must not be older than the oldest age.'
          : 'Ages must be between 14 and 100.'
    }
  }
  let content: string | null = null
  if ([...draft.body].length > MAX_BODY) {
    content = `Your introduction must be ${MAX_BODY} characters or fewer.`
  } else if (publishing && draft.body.trim().length === 0) {
    content = 'Write something before publishing.'
  } else if ([...draft.currentlySeeking].length > MAX_SINGLE_LINE) {
    content = `What you are looking for must be ${MAX_SINGLE_LINE} characters or fewer.`
  } else if ([...draft.interactionStyle].length > MAX_SINGLE_LINE) {
    content = `Your interaction style must be ${MAX_SINGLE_LINE} characters or fewer.`
  } else if (draft.attachmentMediaIds.length > 4) {
    content = 'You can attach up to four photographs.'
  }
  return Object.freeze({
    audience,
    content,
    review: null,
    valid: audience === null && content === null,
  })
}

/** Whether the composer may move to the review step. */
export function canReview(state: ComposerState): boolean {
  const validation = validate(state.draft, false)
  return validation.audience === null && validation.content === null
}

/**
 * Whether the composer may leave the audience step.
 *
 * An invalid audience blocks progress at the step that owns it, not three steps
 * later: a user who inverted the range should learn it while they can still see
 * the two inputs.
 */
export function canLeaveAudience(state: ComposerState): boolean {
  return validate(state.draft, false).audience === null
}

/** The index of a step. */
export function stepIndex(step: Step): number {
  return STEPS.indexOf(step)
}

function withDraft(state: ComposerState, patch: Partial<ComposerDraft>): ComposerState {
  // Any edit clears a stale message, which described the previous draft. The
  // server summary is kept: it describes the *stored* audience, and the review
  // step only shows it after a save, which refreshes it anyway.
  return Object.freeze({
    ...state,
    draft: Object.freeze({ ...state.draft, ...patch }),
    message: '',
  })
}

export function reduce(state: ComposerState, action: Action): ComposerState {
  switch (action.type) {
    case 'go':
      // Moving forward is gated: an invalid audience blocks leaving its step,
      // and an invalid draft blocks the review step.
      if (stepIndex(action.step) > stepIndex(state.step)) {
        if (state.step === 'audience' && !canLeaveAudience(state)) return state
        if (action.step === 'review' && !canReview(state)) return state
      }
      return Object.freeze({ ...state, step: action.step, message: '' })
    case 'next': {
      const index = stepIndex(state.step)
      if (index >= STEPS.length - 1) return state
      const next = STEPS[index + 1]!
      if (state.step === 'audience' && !canLeaveAudience(state)) return state
      if (next === 'review' && !canReview(state)) return state
      return Object.freeze({ ...state, step: next, message: '' })
    }
    case 'back': {
      const index = stepIndex(state.step)
      if (index <= 0) return state
      return Object.freeze({ ...state, step: STEPS[index - 1]!, message: '' })
    }
    case 'gender':
      return withDraft(state, { gender: action.value })
    case 'ageEnabled':
      return withDraft(state, {
        age: Object.freeze({ ...state.draft.age, enabled: action.value }),
      })
    case 'ageMinimum':
      return withDraft(state, {
        age: Object.freeze({ ...state.draft.age, minimum: action.value }),
      })
    case 'ageMaximum':
      return withDraft(state, {
        age: Object.freeze({ ...state.draft.age, maximum: action.value }),
      })
    case 'body':
      return withDraft(state, { body: action.value })
    case 'currentlySeeking':
      return withDraft(state, { currentlySeeking: action.value })
    case 'interactionStyle':
      return withDraft(state, { interactionStyle: action.value })
    case 'attachments':
      return withDraft(state, { attachmentMediaIds: Object.freeze([...action.value]) })
    case 'expiresAt':
      return withDraft(state, { expiresAt: action.value })
    case 'saved':
      return Object.freeze({
        ...state,
        introId: action.introId,
        revision: action.revision,
        audienceSummary: action.audienceSummary,
        message: '',
      })
    case 'message':
      return Object.freeze({ ...state, message: action.value })
    case 'reset':
      return initial(action.favoritesSnapshot)
    default:
      return state
  }
}

/** The `social.SaveIntroPost` body for the current draft. */
export function saveBody(state: ComposerState): {
  expected_revision: number
  body: string
  favorites_snapshot: readonly string[]
  currently_seeking: string | null
  interaction_style: string | null
  attachment_media_ids: readonly string[]
  audience: Audience
  expires_at: string | null
} | null {
  const audience = draftAudience(state.draft)
  if (audience === null) return null
  return Object.freeze({
    expected_revision: state.revision,
    body: state.draft.body,
    favorites_snapshot: state.draft.favoritesSnapshot,
    currently_seeking: state.draft.currentlySeeking === '' ? null : state.draft.currentlySeeking,
    interaction_style: state.draft.interactionStyle === '' ? null : state.draft.interactionStyle,
    attachment_media_ids: state.draft.attachmentMediaIds,
    audience,
    expires_at: state.draft.expiresAt,
  })
}

/** The audience the composer starts from when it is empty. */
export const DEFAULT_AUDIENCE: Audience = ANYONE
