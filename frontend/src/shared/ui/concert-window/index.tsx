import { createContext, useContext, useId, type ComponentPropsWithRef, type ReactNode } from 'react'
import { Tabs, type TabsProps } from '../Tabs'
import './concert-window.css'

export type ConcertIconName = 'world' | 'feed' | 'meet' | 'gifts' | 'wall' | 'me' | 'close' | 'star' | 'bell' | 'settings' | 'map' | 'help'

const paths: Record<ConcertIconName, ReactNode> = {
  world: <><path d="m3 11 9-8 9 8M6 10v11h12V10M10 21v-7h4v7" /></>,
  feed: <><path d="M12 12v9M8 21h8M5 5a10 10 0 0 0 0 14M19 5a10 10 0 0 1 0 14M8 8a6 6 0 0 0 0 8M16 8a6 6 0 0 1 0 8" /><circle cx="12" cy="11" r="2" fill="currentColor" /></>,
  meet: <><circle cx="8" cy="7" r="3" /><circle cx="17" cy="9" r="2" /><path d="M2 21v-4a6 6 0 0 1 12 0v4M16 14a5 5 0 0 1 6 5v2" /></>,
  gifts: <><path d="M3 9h18v5H3zM5 14v7h14v-7M12 9v12" /><path d="M12 9C2 9 7-1 12 9c5-10 10 0 0 0Z" /></>,
  wall: <><path d="m6 7 7-4 7 12-7 4ZM8 6l-2-3M17 4l2-2M5 11 2 13M12 18l1 4M9 20l-3 2" /><path d="m11 8 5 8" /></>,
  me: <><circle cx="12" cy="7" r="4" /><path d="M4 22v-3a8 8 0 0 1 16 0v3Z" /></>,
  close: <path d="m6 6 12 12M18 6 6 18" />,
  bell: <><path d="M5 17h14l-2-3V9a5 5 0 0 0-10 0v5ZM10 21h4M12 2v2" /></>,
  settings: <><path d="m9 3 1 3h4l1-3 3 2-1 3 2 3 3 1-1 4h-3l-2 3v3h-4l-1-3-3-1-3 1-2-3 2-2V9L2 7l3-3 3 2Z" /><circle cx="12" cy="12" r="3" /></>,
  map: <><path d="m3 5 6-2 6 2 6-2v16l-6 2-6-2-6 2ZM9 3v16M15 5v16" /></>,
  help: <><circle cx="12" cy="12" r="9" /><path d="M9 8a3 3 0 0 1 6 0c0 3-3 2-3 5M12 16v2" /></>,
  star: <path d="m12 2 3 6 7 1-5 5 1 7-6-3-6 3 1-7-5-5 7-1Z" />,
}

/** Decorative glyph. Its containing link/button supplies the accessible label. */
export function ConcertIcon({ name }: { name: ConcertIconName }) {
  return <svg className="concert-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor"
    strokeWidth="2" strokeLinecap="square" strokeLinejoin="miter" aria-hidden="true" focusable="false">{paths[name]}</svg>
}

export interface ConcertWindowProps {
  title: string
  children: ReactNode
  headingLevel?: 1 | 2 | 3
  icon?: ConcertIconName
  onClose?: () => void
  closeLabel?: string
  footer?: ReactNode
  id?: string
}

const EmbeddedWindow = createContext(false)
/** Root owns the first titlebar; nested feature windows keep their own headings. */
export function ConcertWindowEmbedding({ children }: { children: ReactNode }) {
  return <EmbeddedWindow value={true}>{children}</EmbeddedWindow>
}

/** A labeled in-flow region, not a modal. Its owner handles navigation/focus on close. */
export function ConcertWindow({ title, children, headingLevel = 2, icon = 'star', onClose,
  closeLabel = `Close ${title}`, footer, id }: ConcertWindowProps) {
  const titleId = useId(), embedded = useContext(EmbeddedWindow) && !onClose
  const Heading = headingLevel === 1 ? 'h1' : headingLevel === 3 ? 'h3' : 'h2'
  return <section id={id} className={`concert-kit concert-window${embedded ? ' concert-window--embedded' : ''}`} aria-labelledby={titleId}>
    <header className="concert-window__titlebar">
      <ConcertIcon name={icon} /><Heading id={titleId}>{title}</Heading>
      {onClose && <button className="concert-window__close" type="button" aria-label={closeLabel} onClick={onClose}>
        <ConcertIcon name="close" />
      </button>}
    </header>
    <div className="concert-window__body"><EmbeddedWindow value={false}>{children}</EmbeddedWindow></div>
    {footer && <footer className="concert-window__footer">{footer}</footer>}
  </section>
}

export type ConcertActionProps = ComponentPropsWithRef<'button'> & {
  tone?: 'red' | 'paper'
  pending?: boolean
}

/** Caller owns pending text and all domain outcomes; no request or replay behavior. */
export function ConcertAction({ tone = 'red', pending = false, disabled, type = 'button',
  className = '', ...props }: ConcertActionProps) {
  return <button {...props} type={type} disabled={disabled || pending} aria-busy={pending || undefined}
    className={`concert-kit concert-action concert-action--${tone} ${className}`} />
}

/** Existing automatic-activation keyboard tabs, with scoped concert presentation. */
export function ConcertTabs(props: TabsProps) {
  return <div className="concert-kit concert-tabs"><Tabs {...props} /></div>
}

export interface ConcertNavigationItem {
  id: string
  label: string
  href: string
  icon: ConcertIconName
}

/** Real links supplied by the router owner. No route or permission is inferred here. */
export function ConcertNavigation({ label, items, current }: {
  label: string
  items: readonly ConcertNavigationItem[]
  current?: string
}) {
  return <nav className="concert-kit concert-navigation" aria-label={label}>
    <ul>{items.map(item => <li key={item.id}><a href={item.href} aria-current={current === item.id ? 'page' : undefined}>
      <ConcertIcon name={item.icon} /><span>{item.label}</span>
    </a></li>)}</ul>
  </nav>
}

export interface ConcertRowProps {
  title: ReactNode
  metadata?: ReactNode
  children?: ReactNode
  visual?: ReactNode
  action?: ReactNode
}

/** Place inside a semantic list. Visuals must use approved/authorized assets. */
export function ConcertRow({ title, metadata, children, visual, action }: ConcertRowProps) {
  return <li className="concert-kit concert-row">
    {visual && <div className="concert-row__visual">{visual}</div>}
    <div className="concert-row__copy"><h3>{title}</h3>
      {metadata && <div className="concert-row__metadata">{metadata}</div>}
      {children && <div className="concert-row__description">{children}</div>}
    </div>
    {action && <div className="concert-row__action">{action}</div>}
  </li>
}

/** Explicit owner-supplied state; do not infer success from presentation. */
export function ConcertState({ title, children, kind = 'empty' }: {
  title: string
  children: ReactNode
  kind?: 'empty' | 'unavailable' | 'pending'
}) {
  return <div className={`concert-kit concert-state concert-state--${kind}`} role="status" aria-live="polite">
    <strong>{title}</strong><div>{children}</div>
  </div>
}
