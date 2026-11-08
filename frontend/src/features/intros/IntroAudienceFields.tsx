import { useId } from 'react'
import { GENDERS, MAX_AGE, MIN_AGE, type GenderCondition } from './contract'

/**
 * WP-05 T-407: the audience fields.
 *
 * The heading is **exactly** "Who can view my mutuals-intro post" — not a
 * paraphrase, because the sentence is the product decision: the audience decides
 * who may see the post, not who the author wants to see.
 *
 * Below the heading, one line says that in words, so a user cannot mistake the
 * control for a preference filter.
 *
 * The gender select is the closed four-value set. `unknown` is offered because
 * the provider can report "not specified" explicitly; it is not a wildcard and
 * the label says so.
 *
 * The age range is an enable checkbox plus two inclusive number inputs. The
 * conversion to the half-open wire form happens in the reducer, not here, so the
 * component never carries two representations of the same value.
 *
 * No Mantine, no `RangeSlider`, no drag interaction: the controls are a native
 * `<select>` and two `<input type="number">` elements, so the whole step is
 * keyboard operable and every control has a real label.
 */
export interface AudienceFieldsProps {
  readonly gender: GenderCondition
  readonly ageEnabled: boolean
  readonly minimum: string
  readonly maximum: string
  /** The derived error for the age range, or `null`. */
  readonly error: string | null
  readonly onGender: (value: GenderCondition) => void
  readonly onAgeEnabled: (value: boolean) => void
  readonly onMinimum: (value: string) => void
  readonly onMaximum: (value: string) => void
}

/** The exact heading text. Exported so the test asserts the same constant. */
export const AUDIENCE_HEADING = 'Who can view my mutuals-intro post'

/** The one-line explanation under the heading. */
export const AUDIENCE_EXPLANATION =
  'This decides who may see your post. It is not a filter for who you want to meet.'

/** The label for each gender condition. */
export const GENDER_LABEL: Readonly<Record<GenderCondition, string>> = Object.freeze({
  any: 'Anyone',
  female: 'Women only',
  male: 'Men only',
  unknown: 'People whose gender was not specified',
})

export function IntroAudienceFields(props: AudienceFieldsProps) {
  const id = useId()
  const errorId = `${id}-age-error`
  return (
    <fieldset className="intro-audience">
      <legend>
        <h2>{AUDIENCE_HEADING}</h2>
      </legend>
      <p className="intro-audience-explanation">{AUDIENCE_EXPLANATION}</p>
      <label htmlFor={`${id}-gender`}>Gender</label>
      <select
        id={`${id}-gender`}
        value={props.gender}
        onChange={(event) => props.onGender(event.target.value as GenderCondition)}
      >
        {GENDERS.map((value) => (
          <option key={value} value={value}>
            {GENDER_LABEL[value]}
          </option>
        ))}
      </select>
      <div className="intro-audience-age">
        <label htmlFor={`${id}-age-enabled`} className="intro-audience-toggle">
          <input
            id={`${id}-age-enabled`}
            type="checkbox"
            checked={props.ageEnabled}
            onChange={(event) => props.onAgeEnabled(event.target.checked)}
          />
          Limit by age
        </label>
        <IntroAgeRangeControl
          id={id}
          disabled={!props.ageEnabled}
          minimum={props.minimum}
          maximum={props.maximum}
          error={props.error}
          errorId={errorId}
          onMinimum={props.onMinimum}
          onMaximum={props.onMaximum}
        />
      </div>
      {props.error !== null && (
        <p id={errorId} className="intro-audience-error" role="alert">
          {props.error}
        </p>
      )}
    </fieldset>
  )
}

/**
 * The inclusive age range control.
 *
 * Two number inputs, both inclusive, both labelled. The upper input's label says
 * "oldest" rather than "maximum" so a user reads it as inclusive without having
 * to learn the wire form.
 */
export function IntroAgeRangeControl({
  id,
  disabled,
  minimum,
  maximum,
  error,
  errorId,
  onMinimum,
  onMaximum,
}: {
  readonly id: string
  readonly disabled: boolean
  readonly minimum: string
  readonly maximum: string
  readonly error: string | null
  readonly errorId: string
  readonly onMinimum: (value: string) => void
  readonly onMaximum: (value: string) => void
}) {
  const describedBy = error !== null ? errorId : undefined
  return (
    <div className="intro-age-range">
      <label htmlFor={`${id}-minimum`}>Youngest age</label>
      <input
        id={`${id}-minimum`}
        type="number"
        inputMode="numeric"
        min={MIN_AGE}
        max={MAX_AGE}
        step={1}
        disabled={disabled}
        value={minimum}
        aria-invalid={error !== null}
        aria-describedby={describedBy}
        onChange={(event) => onMinimum(event.target.value)}
      />
      <label htmlFor={`${id}-maximum`}>Oldest age</label>
      <input
        id={`${id}-maximum`}
        type="number"
        inputMode="numeric"
        min={MIN_AGE}
        max={MAX_AGE}
        step={1}
        disabled={disabled}
        value={maximum}
        aria-invalid={error !== null}
        aria-describedby={describedBy}
        onChange={(event) => onMaximum(event.target.value)}
      />
    </div>
  )
}
