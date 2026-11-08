import { describe, expect, it } from 'vitest'
import {
  canReview,
  emptyDraft,
  initial,
  reduce,
  saveBody,
  STEPS,
  stepIndex,
  validate,
  type ComposerState,
} from '../composer-reducer'
import { audienceFromInclusive, inclusiveToHalfOpen, MAX_BODY } from '../contract'

function withAge(minimum: string, maximum: string): ComposerState {
  let state = initial()
  state = reduce(state, { type: 'ageEnabled', value: true })
  state = reduce(state, { type: 'ageMinimum', value: minimum })
  state = reduce(state, { type: 'ageMaximum', value: maximum })
  return state
}

describe('composer reducer', () => {
  it('starts on the audience step in the product order', () => {
    expect(STEPS).toEqual(['audience', 'content', 'review'])
    expect(initial().step).toBe('audience')
    expect(stepIndex('audience')).toBe(0)
    expect(stepIndex('review')).toBe(2)
  })

  // The acceptance case: moving back and forward preserves every field.
  it('preserves every field across step navigation', () => {
    let state = initial()
    state = reduce(state, { type: 'gender', value: 'female' })
    state = reduce(state, { type: 'ageEnabled', value: true })
    state = reduce(state, { type: 'ageMinimum', value: '18' })
    state = reduce(state, { type: 'ageMaximum', value: '29' })
    state = reduce(state, { type: 'next' })
    expect(state.step).toBe('content')
    state = reduce(state, { type: 'body', value: 'A real introduction.' })
    state = reduce(state, { type: 'currentlySeeking', value: 'Someone to see shows with' })
    state = reduce(state, { type: 'interactionStyle', value: 'Slow replies, long messages' })
    state = reduce(state, { type: 'attachments', value: ['a', 'b'] })
    state = reduce(state, { type: 'next' })
    expect(state.step).toBe('review')
    // Back all the way and forward again: nothing was lost.
    state = reduce(state, { type: 'back' })
    state = reduce(state, { type: 'back' })
    expect(state.step).toBe('audience')
    expect(state.draft.gender).toBe('female')
    expect(state.draft.age).toEqual({ enabled: true, minimum: '18', maximum: '29' })
    state = reduce(state, { type: 'next' })
    expect(state.draft.body).toBe('A real introduction.')
    expect(state.draft.currentlySeeking).toBe('Someone to see shows with')
    expect(state.draft.interactionStyle).toBe('Slow replies, long messages')
    expect(state.draft.attachmentMediaIds).toEqual(['a', 'b'])
  })

  // The acceptance case: an invalid audience blocks the review step.
  it('blocks the review step when the audience is invalid', () => {
    // An inverted range.
    let state = withAge('30', '18')
    expect(validate(state.draft, false).audience).toContain('youngest age')
    expect(canReview(state)).toBe(false)
    // Continue from the audience step is refused, so the step does not move.
    const blocked = reduce(state, { type: 'next' })
    expect(blocked.step).toBe('audience')
    // And from the content step it cannot reach review either.
    const atContent = reduce({ ...state, step: 'content' }, { type: 'next' })
    expect(atContent.step).toBe('content')
    // Jumping straight to review is refused too.
    expect(reduce(state, { type: 'go', step: 'review' }).step).toBe('audience')
    // A half-typed range is also invalid.
    state = withAge('18', '')
    expect(canReview(state)).toBe(false)
    state = withAge('', '29')
    expect(canReview(state)).toBe(false)
    // A valid range passes.
    state = withAge('18', '29')
    expect(canReview(state)).toBe(true)
    expect(reduce(state, { type: 'next' }).step).toBe('content')
  })

  it('accepts a single-year range and the window boundaries', () => {
    expect(canReview(withAge('18', '18'))).toBe(true)
    expect(canReview(withAge('14', '100'))).toBe(true)
    // Out of window.
    expect(canReview(withAge('13', '30'))).toBe(false)
    expect(canReview(withAge('18', '101'))).toBe(false)
  })

  it('rejects a non-integer age', () => {
    for (const [minimum, maximum] of [
      ['18.5', '29'],
      ['+18', '29'],
      ['-18', '29'],
      ['eighteen', '29'],
    ] as const) {
      expect(canReview(withAge(minimum, maximum))).toBe(false)
    }
  })

  // Validation is derived, not stored: the same draft always yields the same
  // answer, and there is no flag that can go stale.
  it('derives validation from the draft rather than storing it', () => {
    const state = withAge('30', '18')
    expect(validate(state.draft, false)).toEqual(validate(state.draft, false))
    // Fixing the range fixes the answer with no extra action.
    const fixed = reduce(state, { type: 'ageMaximum', value: '40' })
    expect(canReview(fixed)).toBe(true)
    // The state carries no validity flag.
    expect(Object.keys(state)).not.toContain('valid')
    expect(Object.keys(state)).not.toContain('errors')
  })

  it('blocks publishing an empty body but allows saving a draft', () => {
    const state = withAge('18', '29')
    expect(validate(state.draft, false).content).toBeNull()
    expect(validate(state.draft, true).content).toBe('Write something before publishing.')
    const written = reduce(state, { type: 'body', value: '   ' })
    expect(validate(written.draft, true).content).toBe('Write something before publishing.')
    const real = reduce(written, { type: 'body', value: 'Hello.' })
    expect(validate(real.draft, true).content).toBeNull()
  })

  it('bounds the body at 1000 code points', () => {
    const state = reduce(initial(), { type: 'body', value: 'a'.repeat(MAX_BODY) })
    expect(validate(state.draft, true).content).toBeNull()
    const over = reduce(initial(), { type: 'body', value: 'a'.repeat(MAX_BODY + 1) })
    expect(validate(over.draft, true).content).toContain('1000')
    // The bound is in code points, not bytes.
    const emoji = reduce(initial(), { type: 'body', value: '\u{1F600}'.repeat(MAX_BODY) })
    expect(validate(emoji.draft, true).content).toBeNull()
  })

  it('sends the half-open audience in the save body', () => {
    let state = withAge('18', '29')
    state = reduce(state, { type: 'gender', value: 'female' })
    state = reduce(state, { type: 'body', value: 'Hello.' })
    const body = saveBody(state)
    expect(body).not.toBeNull()
    expect(body!.audience).toEqual({
      allowed_gender: 'female',
      minimum_age: 18,
      maximum_age_exclusive: 30,
    })
    expect(body!.expected_revision).toBe(0)
    // An empty optional field is sent as null, not as an empty string.
    expect(body!.currently_seeking).toBeNull()
    expect(body!.interaction_style).toBeNull()
  })

  it('sends no age bounds when the range is disabled', () => {
    const state = reduce(initial(), { type: 'gender', value: 'male' })
    expect(saveBody(state)!.audience).toEqual({
      allowed_gender: 'male',
      minimum_age: null,
      maximum_age_exclusive: null,
    })
    // A save body is refused while the audience is invalid.
    expect(saveBody(withAge('30', '18'))).toBeNull()
  })

  it('records the server summary and the revision after a save', () => {
    let state = initial()
    state = reduce(state, {
      type: 'saved',
      introId: '00000000-0000-4000-8000-000000000001',
      revision: 3,
      audienceSummary: 'Women \u00b7 Ages 18\u201329',
    })
    expect(state.introId).toBe('00000000-0000-4000-8000-000000000001')
    expect(state.revision).toBe(3)
    expect(state.audienceSummary).toBe('Women \u00b7 Ages 18\u201329')
    // The next save uses the stored revision.
    expect(saveBody(state)!.expected_revision).toBe(3)
  })

  it('clears a stale message on the next edit', () => {
    let state = reduce(initial(), { type: 'message', value: 'That did not save.' })
    expect(state.message).toBe('That did not save.')
    state = reduce(state, { type: 'body', value: 'edited' })
    expect(state.message).toBe('')
  })

  it('resets to an empty draft with the given favorites', () => {
    let state = reduce(initial(), { type: 'body', value: 'gone' })
    state = reduce(state, { type: 'reset', favoritesSnapshot: ['item-1'] })
    expect(state.draft.body).toBe('')
    expect(state.draft.favoritesSnapshot).toEqual(['item-1'])
    expect(state).toEqual(initial(['item-1']))
  })

  it('does not go back before the first step or past the last', () => {
    const first = initial()
    expect(reduce(first, { type: 'back' })).toBe(first)
    let state = reduce(reduce(first, { type: 'next' }), { type: 'next' })
    expect(state.step).toBe('review')
    expect(reduce(state, { type: 'next' })).toBe(state)
  })

  it('converts an inclusive range to the half-open wire form', () => {
    expect(inclusiveToHalfOpen(18, 29)).toEqual({ minimum_age: 18, maximum_age_exclusive: 30 })
    expect(audienceFromInclusive('any', { minimum: 18, maximumInclusive: 29 })).toEqual({
      allowed_gender: 'any',
      minimum_age: 18,
      maximum_age_exclusive: 30,
    })
    expect(emptyDraft().age.enabled).toBe(false)
  })
})
