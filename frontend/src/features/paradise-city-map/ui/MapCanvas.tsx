import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import type { LandmarkId } from '../spatial/contract'
import type { PersonView, PlaceNote, VenueView } from '../ports'
import type { Selection } from '../model/controller'

export function clusterPeople(people: readonly PersonView[], scale: number) {
  const groups: PersonView[][] = []
  for (const person of [...people].filter(p => p.point).sort((a, b) => a.userId.localeCompare(b.userId))) {
    const group = groups.find(g => Math.hypot((g[0]!.point!.x - person.point!.x) * scale, (g[0]!.point!.y - person.point!.y) * scale) < 48)
    if (group) group.push(person); else groups.push([person])
  }
  return groups
}
export function MapCanvas({ venue, people, self, notes, selection, onSelect, renderPortrait, focusLandmark, compact }: {
  readonly venue: VenueView; readonly people: readonly PersonView[]; readonly self: PersonView | null
  readonly notes: readonly PlaceNote[]; readonly selection: Selection | null; readonly compact: boolean
  readonly onSelect: (selection: Selection) => void
  readonly renderPortrait: (person: Pick<PersonView, 'displayName' | 'avatar'>, size: 64 | 128) => ReactNode
  readonly focusLandmark: { readonly id: LandmarkId; readonly serial: number } | null
}) {
  const scroller = useRef<HTMLDivElement>(null), previous = useRef<{ width: number; height: number; slotWidth: number; slotHeight: number; left: number; top: number } | null>(null)
  const [slot, setSlot] = useState({ width: 390, height: 500 }), [zoom, setZoom] = useState<number | 'fit' | 'read'>('read')
  const drag = useRef<{ id: number; x: number; y: number; left: number; top: number } | null>(null)
  const fit = Math.min(slot.width / venue.image.width, slot.height / venue.image.height)
  const scale = zoom === 'fit' ? fit : zoom === 'read' ? Math.max(slot.width / venue.image.width, slot.height / venue.image.height) : Math.max(fit, Math.min(fit * 8, zoom))
  const width = venue.image.width * scale, height = venue.image.height * scale
  useLayoutEffect(() => {
    const element = scroller.current
    if (!element) return
    const measure = () => setSlot({ width: Math.max(1, element.clientWidth), height: Math.max(1, element.clientHeight) })
    measure(); const observer = new ResizeObserver(measure); observer.observe(element); return () => observer.disconnect()
  }, [])
  useLayoutEffect(() => {
    const element = scroller.current
    if (!element) return
    const before = previous.current
    const x = before ? (before.left + before.slotWidth / 2) / before.width : .5
    const y = before ? (before.top + before.slotHeight / 2) / before.height : .5
    element.scrollLeft = Math.max(0, x * width - slot.width / 2); element.scrollTop = Math.max(0, y * height - slot.height / 2)
    previous.current = { width, height, slotWidth: slot.width, slotHeight: slot.height, left: element.scrollLeft, top: element.scrollTop }
  }, [width, height, slot.width, slot.height])
  useLayoutEffect(() => {
    if (!focusLandmark) return
    const element = scroller.current?.querySelector<HTMLButtonElement>(`[data-landmark-id="${CSS.escape(focusLandmark.id)}"]`)
    element?.focus({ preventScroll: true }); if (element) reveal(element)
  }, [focusLandmark])
  function reveal(element: HTMLElement) {
    const host = scroller.current; if (!host) return
    const r = element.getBoundingClientRect(), v = host.getBoundingClientRect(), margin = 8
    if (r.left < v.left + margin) host.scrollLeft += r.left - v.left - margin
    else if (r.right > v.right - margin) host.scrollLeft += r.right - v.right + margin
    if (r.top < v.top + margin) host.scrollTop += r.top - v.top - margin
    else if (r.bottom > v.bottom - margin) host.scrollTop += r.bottom - v.bottom + margin
  }
  const groups = clusterPeople(people, scale)
  const noteGroups = venue.landmarks.map(place => ({ place, notes: notes.filter(note => note.landmarkId === place.id) })).filter(group => group.notes.length)
  return <div className={`pc-map-canvas${compact ? ' pc-map-canvas--compact' : ''}`}>
    <div className="pc-map-viewport" ref={scroller} tabIndex={0} role="region" aria-label="Paradise City siteplan; arrow keys or drag to pan"
      onScroll={event => { if (previous.current) { previous.current.left = event.currentTarget.scrollLeft; previous.current.top = event.currentTarget.scrollTop } }}
      onFocusCapture={event => { if (event.target !== event.currentTarget) reveal(event.target as HTMLElement) }}
      onKeyDown={event => {
        if (event.target !== event.currentTarget) return
        const host = event.currentTarget, amount = event.shiftKey ? 160 : 48
        const delta = { ArrowLeft: [-amount, 0], ArrowRight: [amount, 0], ArrowUp: [0, -amount], ArrowDown: [0, amount] }[event.key]
        if (delta) { event.preventDefault(); host.scrollLeft += delta[0]!; host.scrollTop += delta[1]! }
        if (event.key === 'Home' || event.key === 'End') { event.preventDefault(); host.scrollLeft = event.key === 'Home' ? 0 : host.scrollWidth; host.scrollTop = event.key === 'Home' ? 0 : host.scrollHeight }
      }}
      onPointerDown={event => {
        if (event.pointerType === 'touch' || event.button !== 0 || (event.target as HTMLElement).closest('button')) return
        const host = event.currentTarget; drag.current = { id: event.pointerId, x: event.clientX, y: event.clientY, left: host.scrollLeft, top: host.scrollTop }; host.setPointerCapture(event.pointerId)
      }}
      onPointerMove={event => { const d = drag.current; if (d && d.id === event.pointerId) { event.currentTarget.scrollLeft = d.left + d.x - event.clientX; event.currentTarget.scrollTop = d.top + d.y - event.clientY } }}
      onPointerUp={() => { drag.current = null }} onPointerCancel={() => { drag.current = null }}>
      <div className="pc-map-plane" style={{ width, height }}>
        <img src={venue.image.src} width={venue.image.width} height={venue.image.height} alt={venue.image.description} draggable={false} />
        {venue.landmarks.map(place => <button key={place.id} type="button" className={`pc-landmark${selection?.kind === 'landmark' && selection.id === place.id ? ' is-selected' : ''}`}
          data-landmark-id={place.id} aria-label={`Select ${place.label}`} style={{ left: place.point.x * scale, top: place.point.y * scale }} onClick={() => onSelect({ kind: 'landmark', id: place.id })}>
          <span aria-hidden="true">⚑</span><span className="pc-marker-label">{place.label}</span>
        </button>)}
        {groups.map(group => {
          const first = group[0]!, point = first.point!
          return <button type="button" className="pc-person-marker" key={group.map(p => p.userId).join(':')} aria-label={group.length > 1 ? `View cluster of ${group.length} friends` : `View ${first.displayName} on map`}
            style={{ left: point.x * scale, top: point.y * scale }} onClick={() => onSelect(group.length > 1 ? { kind: 'cluster', ids: group.map(p => p.userId) } : { kind: 'friend', id: first.userId })}>
            <span className={group.length > 1 ? 'pc-marker-portraits pc-marker-portraits--cluster' : 'pc-marker-portraits'}>{group.slice(0, 3).map(person => <span key={person.userId}>{renderPortrait(person, 64)}</span>)}</span>
            <span className="pc-marker-label">{group.length > 1 ? `${group.length} friends` : first.displayName}</span>
          </button>
        })}
        {self?.point && <div className="pc-person-marker pc-self-marker" style={{ left: self.point.x * scale, top: self.point.y * scale }} aria-label="Your current shared position">
          <span className="pc-marker-portraits">{renderPortrait(self, 64)}</span><span className="pc-marker-label">You</span>
        </div>}
        {noteGroups.map(({ place, notes: values }) => <button type="button" className={`pc-note-marker${selection?.kind === 'notes' && selection.id === place.id ? ' is-selected' : ''}`} key={place.id}
          aria-label={`${place.label}, ${values.length} ${values.length === 1 ? 'note' : 'notes'}`} style={{ left: place.point.x * scale + 28, top: place.point.y * scale + 40 }} onClick={() => onSelect({ kind: 'notes', id: place.id })}>
          <span aria-hidden="true">✉</span> {values.length}<span className="pc-sr-only"> notes</span>
        </button>)}
      </div>
    </div>
    <div className="pc-map-controls" aria-label="Siteplan view controls">
      <button type="button" aria-label="Zoom in" onClick={() => setZoom(Math.min(fit * 8, scale * 1.35))}>+</button>
      <button type="button" aria-label="Zoom out" onClick={() => setZoom(Math.max(fit, scale / 1.35))}>−</button>
      <button type="button" aria-label="Fit complete siteplan" onClick={() => setZoom('fit')}>⌖</button>
      <button type="button" aria-label="Reset map view" onClick={() => { setZoom('read'); const host = scroller.current; if (host) { host.scrollLeft = Math.max(0, (width - slot.width) / 2); host.scrollTop = Math.max(0, (height - slot.height) / 2) } }}>↺</button>
      <button type="button" aria-label="Recenter on your shared location" disabled={!self?.point} onClick={() => { const host = scroller.current; if (host && self?.point) { host.scrollLeft = Math.max(0, self.point.x * scale - slot.width / 2); host.scrollTop = Math.max(0, self.point.y * scale - slot.height / 2) } }}>◎</button>
    </div>
  </div>
}
