import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { usePorts } from '../../../app/access'
import { ShellLink } from '../../../app/navigation'
import { ConcertIcon } from '../../../shared/ui/concert-window'
import { projectNearby } from '../../world-nearby/projection'
import { photo2Shortcut } from './photo2-world-display-v1'
import type { Photo2ShortcutSnapshot } from '../../../world/reference-scene/photo2-world-display-v1/model'
import type { WorldPresentationRelease } from '../../../world/composition'

const empty = () => null
const noop = () => () => {}

/** Two nonphysical panel links follow the admitted camera; they grant no World interaction. */
export function WorldPhoto2Shortcuts() {
  const { worldAccess, auth } = usePorts()
  const binding = useSyncExternalStore(worldAccess?.subscribe ?? noop, worldAccess?.read ?? empty, empty)
  const presentation = useSyncExternalStore(binding?.presentation?.subscribe ?? noop, binding?.presentation?.read ?? empty, empty)
  const overlay = useRef<HTMLDivElement>(null)
  const [paint, setPaint] = useState<{ source: WorldPresentationRelease; snapshot: Photo2ShortcutSnapshot; left: number; top: number; pending: boolean } | null>(null)
  const valid = () => binding && presentation && !binding.release.signal.aborted && !presentation.request.signal.aborted
    && auth?.session.isCurrent(presentation.request.scope) && auth.privacy.isCurrent(presentation.request.privacyScope)
    && binding.presentation?.read() === presentation
  useEffect(() => {
    let closed = false, frame = 0, previous = ''
    const update = () => {
      if (closed) return
      const current = binding?.presentation?.read()
      const snapshot = current === presentation && current && !current.request.signal.aborted
        && auth?.session.isCurrent(current.request.scope) && auth.privacy.isCurrent(current.request.privacyScope)
        && !binding?.release.signal.aborted ? current.shortcuts?.read() ?? null : null
      const bounds = overlay.current?.getBoundingClientRect()
      const next = snapshot && current && bounds ? { source: current, snapshot, left: bounds.left, top: bounds.top, pending: current.theme.read().pending !== null } : null
      const key = JSON.stringify(next ? [next.snapshot, next.left, next.top, next.pending] : null)
      if (key !== previous) { previous = key; setPaint(next) }
      frame = requestAnimationFrame(update)
    }
    frame = requestAnimationFrame(update)
    return () => { closed = true; cancelAnimationFrame(frame) }
  }, [binding, presentation, auth])
  const current = valid() && paint?.source === presentation ? paint : null
  return <div ref={overlay} className="pi08-photo2-shortcuts" aria-label="World board shortcuts">
    {current?.snapshot.boards.map(board => {
      const r = board.clientRect, viewport = current.snapshot.viewport
      const width = Math.max(44, r.width), height = Math.max(44, r.height)
      const x = r.x + (r.width - width) / 2, y = r.y + (r.height - height) / 2
      if (![x, y, width, height].every(Number.isFinite) || x < viewport.x || y < viewport.y
        || x + width > viewport.x + viewport.width || y + height > viewport.y + viewport.height) return null
      const kind = board.id === 'photo2-shortcut-meet' ? 'MEET' : 'WALL'
      const permission = presentation?.request.bootstrap.capabilities[kind === 'MEET' ? 'meet' : 'wall']
      const shortcut = photo2Shortcut(kind, permission ? { available: permission.available, disabled: false, pending: current.pending } : null)
      const style = { left: x - current.left, top: y - current.top, width, height }
      return shortcut.href ? <ShellLink key={board.id} id={board.id} to={shortcut.href} style={style}
        aria-label={shortcut.label} onClick={event => {
          if (!valid() || !presentation?.request.bootstrap.capabilities[kind === 'MEET' ? 'meet' : 'wall'].available
            || presentation.theme.read().pending !== null || !presentation.shortcuts?.read()) event.preventDefault()
        }}><span>{shortcut.label}</span></ShellLink>
        : <button key={board.id} id={board.id} type="button" disabled style={style} aria-label={`${shortcut.label} unavailable`}><span>{shortcut.label}</span></button>
    })}
  </div>
}

/** Observe already authorized replication only; never fetch a second roster. */
export function WorldContextStrip({ expanded }: { expanded: boolean }) {
  const { worldAccess, auth } = usePorts()
  const binding = useSyncExternalStore(worldAccess?.subscribe ?? noop, worldAccess?.read ?? empty, empty)
  const read = useCallback(() => {
    if (!binding || binding.release.signal.aborted || !auth?.session.isCurrent(binding.release.scope)
      || !auth.privacy.isCurrent(binding.release.privacyScope)) return null
    return binding.replication.read()
  }, [binding, auth])
  const view = useSyncExternalStore(binding?.replication.subscribe ?? noop, read, empty)
  const people = view && binding ? projectNearby(view, binding.release.scope) : []
  const live = view?.phase === 'live' && view.visible && people.length > 0
  const status = live ? `${people.length} in view` : view?.phase === 'switching' ? 'Changing rooms…' : 'Presence unavailable'
  return <section id="pi08-world-context" className="pi08-world-context" aria-label="World context" data-expanded={expanded}>
    <div className="pi08-context-copy">
      <p><ConcertIcon name="map" /><strong>World</strong><span aria-hidden="true">/</span><span>{status}</span></p>
      {expanded && <p className="pi08-context-caption">Virtual presence, not physical arrival.</p>}
    </div>
    <ShellLink id="pi08-context-action" to={live ? '/world/nearby' : '/world/rooms'} className="mcr-button mcr-button--primary pi08-context-action">
      {live ? 'Nearby' : 'Rooms'}
    </ShellLink>
  </section>
}
