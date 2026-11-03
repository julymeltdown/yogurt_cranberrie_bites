// @vitest-environment jsdom
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { People } from '../People'
import { PublicProfile } from '../../profile/PublicProfile'
import { decodePage, decodeProfile, introSchema, profileSchema, querySchema } from '../contract'
import { harness, publicIntro } from './fixtures/harness'
import baseline from './evidence/unicode-length/prerequisite-probe-00ecd45.json'

const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(h => h.dispose()) })
function setup() { const h = harness(); owned.push(h); return h }
async function ready() { const h = setup(); h.controller.start(); await vi.waitFor(() => expect(h.controller.read().page.phase).toBe('ready')); return h }
const profile = () => ({ ...publicIntro().author, display_name: '😀'.repeat(40), bio: '😀'.repeat(160) })
const intro = () => ({ ...publicIntro(), text: '😀'.repeat(1000), author: profile() })
describe('coupled production read-path Unicode correction', () => {
  it('retains all 16 actual old-module cases and agrees with an independent frozen C03/C04 oracle', () => {
    expect(baseline.cases).toHaveLength(16)
    expect(baseline.cases.filter(row => row.actual !== row.schema_valid)).toHaveLength(8)
    const rows = baseline.cases.map(row => {
      const text = '😀'.repeat(row.codepoints)
      return row.kind === 'intro' ? { ref: 'https://contracts.mcr.invalid/domain/v1/social.schema.json#/$defs/IntroPublic', value: { ...publicIntro(), text } }
        : row.kind === 'query' ? { ref: 'https://contracts.mcr.invalid/domain/v1/social.schema.json#/$defs/IntroQuery', value: { q: text, limit: 20 } }
          : { ref: 'https://contracts.mcr.invalid/core/v1/core.schema.json#/$defs/PublicProfile', value: { ...publicIntro().author, [row.kind]: text } }
    })
    const expected = JSON.parse(execFileSync('python3', [resolve('src/features/people/__tests__/evidence/unicode-length/contract-oracle.py')], { input: JSON.stringify(rows), encoding: 'utf8' }))
    expect(expected).toEqual(baseline.cases.map(row => row.schema_valid))
    const actual = rows.map((row, index) => baseline.cases[index]!.kind === 'intro' ? introSchema.safeParse(row.value).success
      : baseline.cases[index]!.kind === 'query' ? querySchema.safeParse((row.value as { q: string }).q).success : profileSchema.safeParse(row.value).success)
    expect(actual).toEqual(expected)
  })
  it('renders contract-valid discovery and direct profile text through the actual controller and views', async () => {
    const h = setup(), row = intro()
    h.respond(async request => Response.json(new URL(request.url).pathname.endsWith('/intros') ? { items: [row], next_cursor: null } : row.author))
    const view = render(<MemoryRouter><People controller={h.controller} /></MemoryRouter>)
    expect(await screen.findByRole('link', { name: row.author.display_name })).toBeTruthy()
    expect(screen.getByText(row.text)).toBeTruthy()
    expect(h.controller.read().page.data?.items[0]?.text).toBe(row.text)
    view.rerender(<MemoryRouter><PublicProfile controller={h.controller} userId={row.author.user_id} /></MemoryRouter>)
    expect(await screen.findByRole('heading', { name: row.author.display_name })).toBeTruthy()
    expect(screen.getByText(row.author.bio)).toBeTruthy()
    expect(h.requests.every(request => request.method === 'GET')).toBe(true)
    expect(h.controller.read().profile.data?.bio).toBe(row.author.bio)
  })
  it.each(['intro', 'display_name', 'bio'])('rejects over-limit %s without weakening actual context/secrecy decoding', async field => {
    const h = await ready(), row = intro()
    if (field === 'intro') row.text += '😀'; else if (field === 'display_name') row.author.display_name += '😀'; else row.author.bio += '😀'
    expect(() => decodePage({ items: [row], next_cursor: null }, h.privacy.capture())).toThrow()
    if (field !== 'intro') expect(() => decodeProfile(row.author, row.author.user_id)).toThrow()
    h.respond(async () => Response.json({ items: [row], next_cursor: null })); await h.controller.load()
    expect(h.controller.read().page.phase).toBe('error')
    expect(h.controller.read().page.data).toBeNull()
  })
  it('retains strict extra-field, nullable, trim, control-character, order and context semantics', async () => {
    const h = await ready(), row = intro()
    expect(profileSchema.parse({ ...row.author, bio: null }).bio).toBeNull()
    expect(querySchema.parse('\t  😀 \n')).toBe('😀')
    expect(querySchema.parse('   ')).toBe('')
    for (const text of ['a\0b', 'a\nb', 'a\u007fb']) expect(querySchema.safeParse(text).success).toBe(false)
    expect(querySchema.parse('\u200b')).toBe('\u200b')
    expect(() => decodePage({ items: [{ ...row, age: 20 }], next_cursor: null }, h.privacy.capture())).toThrow()
    expect(() => decodeProfile({ ...row.author, handle: 'secret' }, row.author.user_id)).toThrow()
    expect(() => decodeProfile(row.author, '00000000-0000-4000-8000-000000000099')).toThrow()
    expect(() => decodePage({ items: [{ ...row, event_id: '00000000-0000-4000-8000-000000000099' }], next_cursor: null }, h.privacy.capture())).toThrow()
    expect(() => decodePage({ items: [row, row], next_cursor: null }, h.privacy.capture())).toThrow()
  })
  it.each([51, 100, 101])('preserves and canonically submits or rejects a %i-emoji search without extra GET', async count => {
    const h = await ready(); render(<MemoryRouter><People controller={h.controller} /></MemoryRouter>)
    const input = screen.getByLabelText('Find introductions') as HTMLInputElement, text = '😀'.repeat(count), before = h.requests.length
    expect(input.hasAttribute('maxlength')).toBe(false)
    fireEvent.change(input, { target: { value: text } })
    expect(input.value).toBe(text); expect(h.controller.read().input).toBe(text)
    expect(input.getAttribute('aria-invalid')).toBe(String(count > 100))
    fireEvent.click(screen.getByRole('button', { name: 'Search' }))
    if (count <= 100) { await vi.waitFor(() => expect(h.requests).toHaveLength(before + 1)); expect(new URL(h.requests.at(-1)!.url).searchParams.get('q')).toBe(text) }
    else { await act(async () => {}); expect(h.requests).toHaveLength(before); expect(screen.getByRole('alert').textContent).toContain('Shorten your search'); expect(h.controller.read().input).toBe(text); expect(h.controller.read().query).toBe('') }
  })
  it('preserves an IME/over-limit draft, uses trimmed code-point length, and recovers without silent truncation', async () => {
    const h = await ready(); render(<MemoryRouter><People controller={h.controller} /></MemoryRouter>)
    const input = screen.getByLabelText('Find introductions') as HTMLInputElement, text = '😀'.repeat(100)+'한'
    fireEvent.compositionStart(input); fireEvent.change(input, { target: { value: text } }); fireEvent.compositionEnd(input, { data: '한' })
    expect(input.value).toBe(text); expect(h.controller.setQuery(h.controller.read().input)).toBe(false)
    const valid = '  '+'😀'.repeat(50)+'e\u0301'.repeat(25)+'  '
    fireEvent.change(input, { target: { value: valid } }); expect(input.value).toBe(valid)
    expect(screen.getByText('100 / 100 Unicode code points after trimming.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Search' }))
    await vi.waitFor(() => expect(h.controller.read().page.phase).toBe('ready'))
    expect(new URL(h.requests.at(-1)!.url).searchParams.get('q')).toBe(valid.trim())
    expect(input.value).toBe(valid.trim())
  })
})
