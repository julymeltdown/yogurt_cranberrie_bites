// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import {
  AUDIENCE_EXPLANATION,
  AUDIENCE_HEADING,
  IntroAudienceFields,
} from '../IntroAudienceFields'
import { GENDERS, audienceFromInclusive, inclusiveToHalfOpen } from '../contract'

afterEach(cleanup)

/** A controlled harness that mirrors how the composer wires the fields. */
function Harness({ initial }: { initial?: { gender?: 'any' | 'female' | 'male' | 'unknown'; enabled?: boolean; minimum?: string; maximum?: string } }) {
  const [gender, setGender] = useState(initial?.gender ?? 'any')
  const [enabled, setEnabled] = useState(initial?.enabled ?? false)
  const [minimum, setMinimum] = useState(initial?.minimum ?? '')
  const [maximum, setMaximum] = useState(initial?.maximum ?? '')
  const range = enabled && minimum !== '' && maximum !== ''
    ? audienceFromInclusive(gender, { minimum: Number(minimum), maximumInclusive: Number(maximum) })
    : audienceFromInclusive(gender, null)
  const error = enabled && range === null ? 'The youngest age must not be older than the oldest age.' : null
  return (
    <IntroAudienceFields
      gender={gender}
      ageEnabled={enabled}
      minimum={minimum}
      maximum={maximum}
      error={error}
      onGender={setGender}
      onAgeEnabled={setEnabled}
      onMinimum={setMinimum}
      onMaximum={setMaximum}
    />
  )
}

describe('IntroAudienceFields', () => {
  // The acceptance case: the heading text is exactly as specified.
  it('uses the exact heading text', () => {
    render(<Harness />)
    expect(AUDIENCE_HEADING).toBe('Who can view my mutuals-intro post')
    expect(screen.getByRole('heading', { name: 'Who can view my mutuals-intro post' })).toBeTruthy()
    // The heading is inside the fieldset's legend, so the group is named by it.
    const group = screen.getByRole('group', { name: 'Who can view my mutuals-intro post' })
    expect(group).toBeTruthy()
  })

  // The acceptance case: the explanation sentence is present.
  it('explains that the audience decides who may see the post', () => {
    render(<Harness />)
    expect(AUDIENCE_EXPLANATION).toBe(
      'This decides who may see your post. It is not a filter for who you want to meet.',
    )
    expect(screen.getByText(AUDIENCE_EXPLANATION)).toBeTruthy()
  })

  it('offers exactly the four closed gender values', () => {
    render(<Harness />)
    const select = screen.getByLabelText('Gender') as HTMLSelectElement
    expect([...select.options].map((option) => option.value)).toEqual([...GENDERS])
    expect(GENDERS).toEqual(['any', 'female', 'male', 'unknown'])
    // `unknown` is labelled as "not specified", never as a wildcard.
    expect([...select.options].map((option) => option.textContent)).toContain(
      'People whose gender was not specified',
    )
  })

  it('changes the gender condition', () => {
    render(<Harness />)
    const select = screen.getByLabelText('Gender') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'unknown' } })
    expect(select.value).toBe('unknown')
  })

  // The acceptance case: selecting 18–29 sends minimum_age 18 and
  // maximum_age_exclusive 30.
  it('converts an inclusive range to the half-open wire form', () => {
    render(<Harness />)
    fireEvent.click(screen.getByLabelText('Limit by age'))
    fireEvent.change(screen.getByLabelText('Youngest age'), { target: { value: '18' } })
    fireEvent.change(screen.getByLabelText('Oldest age'), { target: { value: '29' } })
    expect(inclusiveToHalfOpen(18, 29)).toEqual({ minimum_age: 18, maximum_age_exclusive: 30 })
    expect(audienceFromInclusive('any', { minimum: 18, maximumInclusive: 29 })).toEqual({
      allowed_gender: 'any',
      minimum_age: 18,
      maximum_age_exclusive: 30,
    })
    // The inputs show the inclusive values the author typed.
    expect((screen.getByLabelText('Youngest age') as HTMLInputElement).value).toBe('18')
    expect((screen.getByLabelText('Oldest age') as HTMLInputElement).value).toBe('29')
  })

  // The acceptance case: an inverted range shows an inline error.
  it('shows an inline error for an inverted range', () => {
    render(<Harness initial={{ enabled: true, minimum: '30', maximum: '18' }} />)
    const alert = screen.getByRole('alert')
    expect(alert.textContent).toContain('youngest age')
    // Both inputs are marked invalid and reference the message.
    for (const label of ['Youngest age', 'Oldest age']) {
      const input = screen.getByLabelText(label)
      expect(input.getAttribute('aria-invalid')).toBe('true')
      expect(input.getAttribute('aria-describedby')).toBe(alert.id)
    }
  })

  it('disables the age inputs until the toggle is on', () => {
    render(<Harness />)
    expect((screen.getByLabelText('Youngest age') as HTMLInputElement).disabled).toBe(true)
    fireEvent.click(screen.getByLabelText('Limit by age'))
    expect((screen.getByLabelText('Youngest age') as HTMLInputElement).disabled).toBe(false)
  })

  it('uses native controls only: no slider and no drag interaction', () => {
    render(<Harness initial={{ enabled: true }} />)
    // No Mantine, no RangeSlider: there is no slider role anywhere.
    expect(screen.queryByRole('slider')).toBeNull()
    expect(document.querySelector('[class*="mantine"]')).toBeNull()
    expect(document.querySelector('[draggable="true"]')).toBeNull()
    // Every control is a native element.
    expect((screen.getByLabelText('Gender') as HTMLElement).tagName).toBe('SELECT')
    expect((screen.getByLabelText('Youngest age') as HTMLElement).tagName).toBe('INPUT')
    expect((screen.getByLabelText('Limit by age') as HTMLElement).getAttribute('type')).toBe('checkbox')
  })

  // Keyboard operation: every control is reachable and changeable without a
  // pointer.
  it('is operable from the keyboard', () => {
    render(<Harness />)
    const select = screen.getByLabelText('Gender') as HTMLSelectElement
    select.focus()
    expect(document.activeElement).toBe(select)
    // A native select changes with the keyboard.
    fireEvent.change(select, { target: { value: 'female' } })
    expect(select.value).toBe('female')
    const toggle = screen.getByLabelText('Limit by age') as HTMLInputElement
    toggle.focus()
    expect(document.activeElement).toBe(toggle)
    fireEvent.click(toggle)
    const minimum = screen.getByLabelText('Youngest age') as HTMLInputElement
    minimum.focus()
    expect(document.activeElement).toBe(minimum)
    fireEvent.change(minimum, { target: { value: '18' } })
    expect(minimum.value).toBe('18')
    // Every control is a labelled form element, so a screen reader announces it.
    for (const name of ['Gender', 'Limit by age', 'Youngest age', 'Oldest age']) {
      expect(screen.getByLabelText(name)).toBeTruthy()
    }
  })

  it('bounds the age inputs to the configured window', () => {
    render(<Harness initial={{ enabled: true }} />)
    for (const label of ['Youngest age', 'Oldest age']) {
      const input = screen.getByLabelText(label)
      expect(input.getAttribute('min')).toBe('14')
      expect(input.getAttribute('max')).toBe('100')
      expect(input.getAttribute('step')).toBe('1')
    }
  })
})
