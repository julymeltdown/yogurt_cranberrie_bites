import { describe, expect, it } from 'vitest'
import { boundedText, codePointLength, tagArray, tagSchema } from '../text'
import { parseTagInput } from '../tags'

describe('audited Unicode and tag utilities', () => {
  it.each([['😀', 1], ['e\u0301', 2], ['ＡＢ', 2], ['a\u200bb', 3], ['👩‍💻', 3]])('counts code points in %s', (text, count) => {
    expect(codePointLength(text)).toBe(count)
    expect(boundedText(count, count).safeParse(text).success).toBe(true)
    expect(boundedText(0, count - 1).safeParse(text).success).toBe(false)
  })
  it('preserves text and does not impose a blanket contact/link ban', () => {
    const text = 'https://example.org @friend 123456789'
    expect(boundedText(0, 1000).parse(text)).toBe(text)
    expect(tagSchema.parse('@friend')).toBe('@friend')
  })
  it('trims tags, checks raw and normalized bounds, and rejects normalized duplicates', () => {
    expect(tagSchema.parse('  Music  ')).toBe('Music')
    expect(tagSchema.parse('😀'.repeat(24))).toHaveLength(48)
    for (const text of ['', '   ', 'x'.repeat(25), ' '+ 'x'.repeat(24), '😀'.repeat(25)]) expect(tagSchema.safeParse(text).success).toBe(false)
    expect(tagArray(2).safeParse(['a', ' a ']).success).toBe(false)
    expect(tagArray(2).parse(['a', 'A'])).toEqual(['a', 'A'])
    expect(tagArray(1).safeParse(['a', 'b']).success).toBe(false)
    expect(tagSchema.parse('\u200b')).toBe('\u200b')
  })
  it('preserves audited comma parsing, hash removal, stable deduplication and limit', () => {
    expect(parseTagInput(' #music, art, #music, , ##tag, # space, last', 4)).toEqual(['music', 'art', '#tag', ' space'])
    expect(parseTagInput('a,b', 0)).toEqual([])
  })
})
