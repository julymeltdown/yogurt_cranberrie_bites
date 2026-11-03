import { createContext, useContext, type ReactNode } from 'react'
import { unavailableDocuments, type DocumentProvider } from './documents'
const DocumentsContext = createContext<DocumentProvider>(unavailableDocuments)
/** Explicit composition seam; production PublicBoundary uses the unavailable default.
 * Synthetic sources live under __tests__ and must set syntheticPreview=true.
 */
export function PublicDocumentsProvider({ provider, children }: { provider: DocumentProvider; children: ReactNode }) {
  return <DocumentsContext value={provider}>{children}</DocumentsContext>
}
export const usePublicDocuments = () => useContext(DocumentsContext)
