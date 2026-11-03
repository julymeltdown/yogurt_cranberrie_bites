// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { createIntroController } from '../controller'
import { IntroEditor } from '../IntroEditor'
import { decodeOwn, draftSchema } from '../contract'
import { eventId, harness, introId, userId } from '../../people/__tests__/fixtures/harness'
import baseline from '../../../shared/validation/__tests__/evidence/prerequisite-probe-572473f.json'

const disposals: (() => void)[] = []
afterEach(() => { cleanup(); disposals.splice(0).forEach(dispose => dispose()) })
const own = (text = '') => ({ intro_id: introId, event_id: eventId, owner_id: userId, text, age_bounds: { minimum_age: null, maximum_age_exclusive: null }, expires_at: null, created_at: '2026-11-07T08:00:00Z', state: 'DRAFT', revision: 1 })
async function setup() {
  const h = harness(), controller = createIntroController({ ...h.options, now: () => Date.parse('2026-11-07T12:00:00Z') })
  h.respond(async request => request.method === 'GET' ? Response.json(own()) : Response.json({ ...own(), ...await request.json() as object, expected_revision: undefined, revision: 2 }))
  disposals.push(() => { controller.dispose(); h.dispose() })
  render(<MemoryRouter><IntroEditor controller={controller} /></MemoryRouter>)
  await vi.waitFor(() => expect(controller.read().load.phase).toBe('ready'))
  return { h, controller, textarea: screen.getByLabelText('Introduction') as HTMLTextAreaElement }
}
describe('production intro Unicode boundary correction', () => {
  it('retains the original actual-module discrepancy as baseline evidence', () => {
    expect(baseline.mismatching_code_points).toEqual([501, 1000])
    expect(baseline.cases.map(row => row.accepted)).toEqual([true, false, false, false])
    expect(baseline.cases.map(row => row.c04_text_accepted)).toEqual([true, true, true, false])
  })
  it.each([500, 501, 1000, 1001])('validates, decodes and saves %i emoji at the actual consumer', async count => {
    const text = '😀'.repeat(count), accepted = count <= 1000
    expect(draftSchema.safeParse({ text, age_bounds: own().age_bounds, expires_at: null }).success).toBe(accepted)
    const hScope = harness(), scope = hScope.privacy.capture()
    try {
      if (accepted) expect(decodeOwn(own(text), scope).text).toBe(text)
      else expect(() => decodeOwn(own(text), scope)).toThrow()
    } finally { hScope.dispose() }
    const { textarea, h, controller } = await setup()
    expect(textarea.hasAttribute('maxlength')).toBe(false)
    fireEvent.change(textarea, { target: { value: text } })
    expect(textarea.value).toBe(text)
    expect(screen.getByText(`${count} / 1,000 Unicode code points.${accepted ? '' : ' Shorten your introduction before saving.'}`)).toBeTruthy()
    expect(textarea.getAttribute('aria-invalid')).toBe(String(!accepted))
    fireEvent.click(screen.getByRole('button', { name: 'Save introduction' }))
    await vi.waitFor(() => expect(accepted ? controller.read().saved?.revision : controller.read().message).toBe(accepted ? 2 : 'Check your text, complete half-open bounds, expiry and event access before saving.'))
    expect(h.requests.filter(request => request.method === 'PUT')).toHaveLength(accepted ? 1 : 0)
    expect(controller.read().draft.text).toBe(text)
    if (accepted) expect(await h.requests.find(request => request.method === 'PUT')!.json()).toMatchObject({ text, expected_revision: 1 })
  })
  it.each([['e\u0301'.repeat(500), true], ['e\u0301'.repeat(500)+'Ａ', false], ['😀'.repeat(499)+'e\u0301', true], ['Ａ'.repeat(1000), true]])('counts combining marks and mixed BMP text without normalization', (text, accepted) => {
    expect(draftSchema.safeParse({ text, age_bounds: own().age_bounds, expires_at: null }).success).toBe(accepted)
  })
  it('preserves the complete composition draft and recovery to the valid boundary', async () => {
    const { textarea, controller } = await setup(), composing = '😀'.repeat(1000)+'한'
    fireEvent.compositionStart(textarea)
    fireEvent.change(textarea, { target: { value: composing } })
    fireEvent.compositionEnd(textarea, { data: '한' })
    expect(textarea.value).toBe(composing)
    expect(controller.read().draft.text).toBe(composing)
    expect(await controller.save()).toBe(false)
    fireEvent.change(textarea, { target: { value: '😀'.repeat(1000) } })
    expect(textarea.getAttribute('aria-invalid')).toBe('false')
    expect(await controller.save()).toBe(true)
  })
})
