import { z } from 'zod'
import { publicPaths } from '../../../app/routes'

export const documentLabels = { terms: 'Terms', privacy: 'Privacy', safety: 'Safety', contact: 'Contact' } as const
export type DocumentKey = keyof typeof documentLabels
export const isDocumentKey = (value: unknown): value is DocumentKey => typeof value === 'string' && Object.hasOwn(documentLabels, value)
export const documentPath = (key: DocumentKey) => `/legal/${key}`
/** Local presentation input, NOT an HTTP schema or publication/retention authority.
 * A future approved adapter must map the integrator's canonical publication contract.
 */
export interface DocumentView {
  readonly key: DocumentKey
  readonly version: string
  readonly publishedAt: string
  readonly effectiveAt: string
  readonly sections: readonly { readonly heading: string; readonly paragraphs: readonly string[]
    readonly links: readonly { readonly label: string; readonly to: string }[] }[]
}
export type DocumentResult = { readonly status: 'unavailable' } | { readonly status: 'published'; readonly document: DocumentView }
export interface DocumentProvider {
  /** Trusted composition flag, never derived from an untrusted document body. */
  readonly syntheticPreview: boolean
  read(key: DocumentKey, signal: AbortSignal): Promise<unknown>
}
export const unavailableDocuments: DocumentProvider = Object.freeze({ syntheticPreview: false,
  read: async () => Object.freeze({ status: 'unavailable' }) })
const record = (value: unknown): Record<string, unknown> => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid document')
  return value as Record<string, unknown>
}
function only(value: Record<string, unknown>, keys: readonly string[]) {
  if (Object.keys(value).length !== keys.length || !Object.keys(value).every(k => keys.includes(k))) throw new Error('Invalid document')
}
function plain(value: unknown, max: number) {
  if (typeof value !== 'string' || value.length > max * 2 || !value.trim()) throw new Error('Invalid document text')
  const characters = Array.from(value)
  if (characters.length > max || characters.some(c => {
    const code = c.codePointAt(0)!
    return (code < 32 && code !== 9 && code !== 10) || (code >= 127 && code <= 159)
      || (code >= 0x202a && code <= 0x202e) || (code >= 0x2066 && code <= 0x2069)
      || (code >= 0xd800 && code <= 0xdfff)
  })) throw new Error('Invalid document text')
  return value
}
function entries(value: unknown, min: number, max: number): unknown[] {
  if (!Array.isArray(value) || value.length < min || value.length > max) throw new Error('Invalid document list')
  return value
}
export function safeDocumentLink(value: unknown): value is string {
  // External/contact destinations need a reviewed publication/link policy. No guessed allowlist.
  return typeof value === 'string' && (publicPaths.some(path => !path.includes(':') && path === value)
    || Object.keys(documentLabels).some(key => value === documentPath(key as DocumentKey)))
}
function timestamp(value: unknown) {
  const result = plain(value, 40)
  if (!result.endsWith('Z') || !z.iso.datetime().safeParse(result).success) throw new Error('Invalid document date')
  return result
}
export function decodeDocument(value: unknown, expected: DocumentKey): DocumentResult {
  const result = record(value)
  if (result.status === 'unavailable') { only(result, ['status']); return Object.freeze({ status: 'unavailable' }) }
  only(result, ['status', 'document'])
  if (result.status !== 'published') throw new Error('Invalid document state')
  const d = record(result.document)
  only(d, ['key', 'version', 'publishedAt', 'effectiveAt', 'sections'])
  if (d.key !== expected || typeof d.version !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(d.version)) throw new Error('Invalid document revision')
  let characters = 0
  const sections = entries(d.sections, 1, 32).map(value => {
    const section = record(value); only(section, ['heading', 'paragraphs', 'links'])
    const heading = plain(section.heading, 160)
    const paragraphs = entries(section.paragraphs, 1, 32).map(p => plain(p, 4096))
    const links = entries(section.links, 0, 16).map(value => {
      const link = record(value); only(link, ['label', 'to'])
      const label = plain(link.label, 120)
      if (!safeDocumentLink(link.to)) throw new Error('Unreviewed document link')
      return Object.freeze({ label, to: link.to })
    })
    characters += heading.length + paragraphs.reduce((n, p) => n + p.length, 0) + links.reduce((n, l) => n + l.label.length + l.to.length, 0)
    if (characters > 65_536) throw new Error('Document too large')
    return Object.freeze({ heading, paragraphs: Object.freeze(paragraphs), links: Object.freeze(links) })
  })
  return Object.freeze({ status: 'published', document: Object.freeze({ key: expected, version: d.version,
    publishedAt: timestamp(d.publishedAt), effectiveAt: timestamp(d.effectiveAt), sections: Object.freeze(sections) }) })
}
export const formatDocumentDate = (value: string) => new Intl.DateTimeFormat('en-GB', {
  day: '2-digit', month: 'short', year: 'numeric', timeZone: 'Asia/Seoul',
}).format(new Date(value))
