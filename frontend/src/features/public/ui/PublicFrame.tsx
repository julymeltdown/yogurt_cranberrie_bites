import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { documentLabels, documentPath, type DocumentKey } from '../model/documents'
import { ConcertIcon } from '../../../shared/ui/concert-window'
import './public.css'

export function PublicFrame({ children, current, appearance }: { children: ReactNode; current?: DocumentKey; appearance?: 'poster' }) {
  return <div className={`public-entry${appearance === 'poster' ? ' public-entry--poster' : ''}`}>
    <a className="public-skip" href="#public-main">Skip to content</a>
    <header className="public-header">
      <Link className="public-brand" to="/" aria-label="MCR Concert World home">{appearance === 'poster' && <ConcertIcon name="star" />}MCR <span>Concert World</span></Link>
      <Link className="public-text-link" to="/login">Sign in</Link>
    </header>
    <main id="public-main" tabIndex={-1}>{children}</main>
    <footer className="public-footer">
      <p>An independent fan project. Not affiliated with My Chemical Romance or concert organizers.</p>
      <nav aria-label="Public information">{(Object.keys(documentLabels) as DocumentKey[]).map(key =>
        <Link key={key} to={documentPath(key)} aria-current={current === key ? 'page' : undefined}>{documentLabels[key]}</Link>)}</nav>
      <p className="public-footer-signature">Different eras. Same crowd.</p>
    </footer>
  </div>
}
