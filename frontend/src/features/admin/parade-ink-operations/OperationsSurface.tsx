import type { ReactNode } from 'react'
import { ConcertWindow } from '../../../shared/ui/concert-window'
import './operations.css'

export type OperationsView = 'control' | 'event' | 'places' | 'members' | 'content' | 'wall' | 'audit' | 'closure'

/** Presentation only: the enclosed view retains its controller and access boundary. */
export function OperationsSurface({ view, children }: { view: OperationsView; children: ReactNode }) {
  return <div className="pi-operations" data-presentation="parade-ink-operations" data-operations-view={view}>{children}</div>
}

/** A feature region inside the administration page's existing main landmark. */
export function OperationsFrame({ title, children }: { title: string; children: ReactNode }) {
  return <section className="event-ops"><ConcertWindow title={title} headingLevel={1}>{children}</ConcertWindow></section>
}
