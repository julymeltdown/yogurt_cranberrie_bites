import { describe, expect, it } from 'vitest'
import { decodeDocument, formatDocumentDate, safeDocumentLink } from '../model/documents'
import { syntheticDocument } from './fixtures/documents'

describe('local document presentation validation, not a publication contract', () => {
  it('accepts and freezes the conspicuously synthetic view with version and UTC timestamps', () => {
    const result = decodeDocument(syntheticDocument(), 'terms')
    expect(result.status).toBe('published')
    expect(Object.isFrozen(result)).toBe(true)
    expect(formatDocumentDate('2026-11-03T23:00:00Z')).toBe('04 Nov 2026')
  })
  it('accepts only the exact unavailable view and never accepts a fabricated error body as copy', () => {
    expect(decodeDocument({ status: 'unavailable' }, 'terms')).toEqual({ status: 'unavailable' })
    expect(() => decodeDocument({ status: 'unavailable', secret: 'private' }, 'terms')).toThrow()
    expect(() => decodeDocument({ status: 'error', message: 'raw provider detail' }, 'terms')).toThrow()
  })
  it.each([
    '//evil.example', 'https://evil.example', 'http://mcr.example/legal/privacy', 'javascript:alert(1)',
    'data:text/html,<script>alert(1)</script>', 'mailto:invented@example.com', '/legal/terms?token=secret',
    '/legal/../privacy', '/legal/%70rivacy', '/legal/privacy#x', '/admin', '/api/auth/logout', '/world',
    '\\evil.example', '/legal/privacy\n', '/legal/unknown',
  ])('rejects unreviewed or hostile link %s', to => {
    const source = syntheticDocument(); source.document.sections[0]!.links[0]!.to = to
    expect(safeDocumentLink(to)).toBe(false)
    expect(() => decodeDocument(source, 'terms')).toThrow()
  })
  it.each(['/legal/terms', '/legal/privacy', '/legal/safety', '/legal/contact', '/login', '/'])('allows existing public destination %s', to => {
    expect(safeDocumentLink(to)).toBe(true)
  })
  it.each(['', 'version with space', '<img>', 'x\n', 'a'.repeat(65)])('rejects invalid version %j', version => {
    const source = syntheticDocument(); source.document.version = version
    expect(() => decodeDocument(source, 'terms')).toThrow()
  })
  it.each(['2026-02-30T00:00:00Z', '2026-11-03', 'yesterday', '2026-11-03T09:00:00+09:00', '<script>'])('rejects invalid publication/effect date %s', value => {
    const source = syntheticDocument(); source.document.publishedAt = value
    expect(() => decodeDocument(source, 'terms')).toThrow()
    source.document.publishedAt = '2026-11-03T00:00:00Z'; source.document.effectiveAt = value
    expect(() => decodeDocument(source, 'terms')).toThrow()
  })
  it('rejects cross-document results, unknown fields, controls, invalid Unicode, emptiness and oversize bodies', () => {
    expect(() => decodeDocument(syntheticDocument('privacy'), 'terms')).toThrow()
    expect(() => decodeDocument({ ...syntheticDocument(), html: '<b>unreviewed</b>' }, 'terms')).toThrow()
    for (const paragraph of ['\u202eevil', '\u0000', '\ud800', ' ', 'x'.repeat(4097)]) {
      const source = syntheticDocument(); source.document.sections[0]!.paragraphs = [paragraph]
      expect(() => decodeDocument(source, 'terms')).toThrow()
    }
    const source = syntheticDocument(); source.document.sections[0]!.paragraphs = Array(32).fill('x'.repeat(4096))
    expect(() => decodeDocument(source, 'terms')).toThrow()
  })
})
