import { useId, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import './pre-world.css'

/** Presentation only: callers retain the authoritative state, copy and actions. */
export function PreWorldFrame({ title, children, actions }: {
  title: ReactNode
  children: ReactNode
  actions: ReactNode
}) {
  const heading = useId()
  return <main className="pi-pre-world" aria-labelledby={heading}>
    <Link className="pi-pre-world__brand" to="/">MCR Concert World</Link>
    <section className="pi-pre-world__window">
      <header className="pi-pre-world__title">
        <h1 id={heading} tabIndex={-1}>{title}</h1>
      </header>
      <div className="pi-pre-world__body">{children}</div>
      <div className="pi-pre-world__actions">{actions}</div>
    </section>
  </main>
}
