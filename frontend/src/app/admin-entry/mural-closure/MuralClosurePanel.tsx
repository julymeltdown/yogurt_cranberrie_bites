import { useEffect, useSyncExternalStore } from 'react'
import { WallControl } from '../../../features/admin/mural'
import { EventClosure } from '../../../features/admin/closure'
import type { MuralClosureEntry, MuralClosureView } from './index'

/** Entry ownership stays with Root so navigation cannot discard an ambiguous command. */
export function MuralClosurePanel({ entry, view }: { entry: MuralClosureEntry; view: MuralClosureView }) {
  useSyncExternalStore(entry.subscribe, entry.read, entry.read)
  useEffect(() => { void entry.start(view) }, [entry, view])
  if (!entry.isCurrent()) return <section className="mcr-ops" aria-label="Event operations unavailable">
    <h1>Event operations unavailable</h1><p role="status">A current authorized operator context is required. Previous controls and private progress are hidden.</p>
  </section>
  return view === 'mural' ? <WallControl controller={entry.wall} /> : <EventClosure controller={entry.closure} />
}
