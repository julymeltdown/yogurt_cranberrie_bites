import type { DocumentKey, DocumentProvider } from '../../model/documents'
export { deferred } from '../../../../test-support/fixtures'
/** Deliberately synthetic UI input. Never imported from the production entry. */
export const syntheticDocument = (key: DocumentKey = 'terms') => ({ status: 'published', document: {
  key, version: 'TEST-2026.11.03', publishedAt: '2026-11-03T00:00:00Z', effectiveAt: '2026-11-04T00:00:00Z',
  sections: [{ heading: 'Synthetic UI qualification document', paragraphs: [
    'SYNTHETIC_FE08_DOCUMENT — this text only tests layout, navigation and version presentation. It is not legal advice, approved terms, privacy policy, safety guidance or operator contact information.',
    'No real retention duration, contact identity, venue directions or eligibility rule is stated by this fixture.',
  ], links: [{ label: 'Open the synthetic privacy page', to: '/legal/privacy' }] }],
} })
export const syntheticDocuments: DocumentProvider = { syntheticPreview: true, read: async key => syntheticDocument(key) }
