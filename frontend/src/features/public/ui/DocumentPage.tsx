import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { usePublicDocuments } from '../model/document-provider'
import { decodeDocument, documentLabels, formatDocumentDate, isDocumentKey, type DocumentProvider, type DocumentResult } from '../model/documents'
import { PublicFrame } from './PublicFrame'

type Load = { status: 'loading' | 'error' } | DocumentResult
export function DocumentPage({ document }: { document: unknown }) {
  const key = isDocumentKey(document) ? document : null, provider = usePublicDocuments()
  const [attempt, setAttempt] = useState(0)
  const [loaded, setLoaded] = useState<{ key: typeof key; provider: DocumentProvider; attempt: number; result: Load } | null>(null)
  const heading = useRef<HTMLHeadingElement>(null)
  useLayoutEffect(() => { heading.current?.focus({ preventScroll: true }) }, [key])
  useEffect(() => {
    if (!key) return
    const controller = new AbortController()
    const commit = (result: Load) => { if (!controller.signal.aborted) setLoaded({ key, provider, attempt, result }) }
    const timer = setTimeout(() => { commit({ status: 'error' }); controller.abort() }, 10_000)
    Promise.resolve().then(() => {
      if (controller.signal.aborted) return undefined
      return provider.read(key, controller.signal)
    }).then(value => {
      if (controller.signal.aborted) return
      commit(decodeDocument(value, key))
    }).catch(() => commit({ status: 'error' })).finally(() => clearTimeout(timer))
    return () => { clearTimeout(timer); controller.abort() }
  }, [key, provider, attempt])
  const state: Load = !key ? { status: 'unavailable' } : loaded?.key === key && loaded.provider === provider && loaded.attempt === attempt
    ? loaded.result : { status: 'loading' }
  const title = key ? documentLabels[key] : 'Document unavailable'
  return <PublicFrame {...(key ? { current: key } : {})}>
    <article className="public-document" aria-busy={state.status === 'loading'}>
      <p className="public-eyebrow">Public information</p>
      <h1 ref={heading} tabIndex={-1}>{title}</h1>
      {state.status === 'loading' && <p role="status">Opening this document…</p>}
      {state.status === 'unavailable' && <p role="status">{key === 'contact' ? 'Operator contact details are not available yet.' : 'This document is not available yet.'}</p>}
      {state.status === 'error' && <div role="alert"><p>We could not open this document. Please try again.</p>
        <button className="public-secondary" type="button" onClick={() => setAttempt(value => value + 1)}>Try again</button></div>}
      {state.status === 'published' && <>
        {provider.syntheticPreview && <p className="public-preview-notice" role="note">Synthetic document preview for UI testing. This is not approved policy or operator contact information.</p>}
        <dl className="public-document-meta">
          <div><dt>Version</dt><dd>{state.document.version}</dd></div>
          <div><dt>Published</dt><dd><time dateTime={state.document.publishedAt}>{formatDocumentDate(state.document.publishedAt)}</time></dd></div>
          <div><dt>Effective</dt><dd><time dateTime={state.document.effectiveAt}>{formatDocumentDate(state.document.effectiveAt)}</time></dd></div>
        </dl>
        <p className="public-date-note">Dates shown in Korea Standard Time.</p>
        {state.document.sections.map((section, index) => <section key={index} className="public-document-section" aria-labelledby={`public-document-section-${index}`}>
          <h2 id={`public-document-section-${index}`}>{section.heading}</h2>
          {section.paragraphs.map((paragraph, index) => <p key={index}>{paragraph}</p>)}
          {section.links.length > 0 && <ul>{section.links.map((link, index) => <li key={index}><Link to={link.to}>{link.label}</Link></li>)}</ul>}
        </section>)}
      </>}
      <Link className="public-text-link public-back" to="/">Back to home</Link>
    </article>
  </PublicFrame>
}
