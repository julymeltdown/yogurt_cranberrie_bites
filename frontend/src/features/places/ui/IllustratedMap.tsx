import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react'
import { ShellLink } from '../../../app/navigation'
import { useNavigationType } from 'react-router-dom'
import { ConcertIcon } from '../../../shared/ui/concert-window'
import type { PlacesController, PlacesState } from '../model/controller'
import { physicalStatus } from '../model/contract'
import { layoutPlaques } from './plaque-layout'

function LandmarkIcon({ interaction }: { interaction: string }) {
  if (/gifts?/.test(interaction)) return <ConcertIcon name="gifts" />
  if (/wall/.test(interaction)) return <ConcertIcon name="wall" />
  if (/park/.test(interaction)) return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m12 2-6 8h3l-5 7h6v5h4v-5h6l-5-7h3Z" /></svg>
  if (/stage/.test(interaction)) return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 21V3h18v18M3 7h18M7 7v10m10-10v10M2 21h20M8 21v-3h8v3M3 3l4 4m14-4-4 4" /></svg>
  return <ConcertIcon name="world" />
}
const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value))
// Exact controller lifetime, weakly held; only a user-selected public place ID.
const selections = new WeakMap<PlacesController, { mapHash: string; id: string }>()

/** Local DOM viewport over the incumbent owner's static pixels. No world camera writes. */
export function IllustratedMap({ controller, state, fallback, legend }: {
  controller: PlacesController; state: PlacesState; fallback: ReactNode; legend: ReactNode
}) {
  const image = state.overview!, map = state.map!, items = state.list.data?.items ?? []
  const viewport = useRef<HTMLDivElement>(null), sheet = useRef<HTMLDialogElement>(null)
  const returnFocus = useRef<HTMLElement | null>(null), drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null)
  const [size, setSize] = useState({ width: 360, height: 600 }), [zoom, setZoom] = useState(1)
  const saved = selections.get(controller), navigation = useNavigationType(), restored = useRef(false)
  const [loaded, setLoaded] = useState(''), [failed, setFailed] = useState(''), [selected, setSelected] = useState<string | null>(saved?.mapHash === map.content_hash ? saved.id : null)
  const [panel, setPanel] = useState<'places' | 'legend' | null>(null), [search, setSearch] = useState('')
  const [reset, setReset] = useState(0)
  const [scroll, setScroll] = useState({ left: 0, top: 0 })
  const ready = loaded === image.imageUrl && failed !== image.imageUrl
  const baseWidth = Math.max(size.width, size.height * image.width / image.height)
  const scale = baseWidth * zoom / image.width, width = image.width * scale, height = image.height * scale
  const minZoom = Math.min(1, size.width / baseWidth)
  const current = items.find(p => p.place_id === selected)
  const supported = !!current && current.closure_state === 'OPEN' && !state.unavailablePlaceIds.includes(current.place_id)
  const target = supported ? map.geometry.interactions.find(p => p.place_id === selected) : null
  const anchors = items.flatMap(place => {
    const point = map.geometry.interactions.find(p => p.place_id === place.place_id)
    return point ? [{ id: place.place_id, x: (point.cell_x + .5) / map.geometry.width_cells * width, y: (point.cell_y + .5) / map.geometry.height_cells * height }] : []
  })
  const plaques = layoutPlaques(anchors, { ...scroll, ...size }, height)
  function select(id: string) { selections.set(controller, { mapHash: map.content_hash, id }); setSelected(id) }
  useEffect(() => {
    if (ready && navigation === 'POP' && !restored.current && saved?.mapHash === map.content_hash) {
      restored.current = true
      document.getElementById(`map-place-${saved.id}`)?.focus()
    }
  }, [ready, navigation, saved, map.content_hash])
  useLayoutEffect(() => {
    const element = viewport.current
    if (!element) return
    const update = () => { const r = element.getBoundingClientRect(); if (r.width > 0 && r.height > 0) setSize({ width: r.width, height: r.height }) }
    update(); const observer = new ResizeObserver(update); observer.observe(element)
    return () => observer.disconnect()
  }, [])
  useLayoutEffect(() => {
    const element = viewport.current
    if (element) { element.scrollLeft = Math.max(0, (width - size.width) / 2); element.scrollTop = Math.max(0, (height - size.height) / 2); setScroll({ left: element.scrollLeft, top: element.scrollTop }) }
  }, [width, height, size.width, size.height, reset])
  useEffect(() => {
    const dialog = sheet.current
    if (!panel || !dialog) return
    dialog.showModal()
    return () => { dialog.close(); if (returnFocus.current?.isConnected) returnFocus.current.focus({ preventScroll: true }) }
  }, [panel])
  function panKey(event: KeyboardEvent<HTMLDivElement>) {
    if (event.target !== event.currentTarget) return
    const direction: Record<string, [number, number]> = { ArrowLeft: [-64, 0], ArrowRight: [64, 0], ArrowUp: [0, -64], ArrowDown: [0, 64] }
    const delta = direction[event.key]
    if (delta) { event.preventDefault(); event.currentTarget.scrollBy(...delta) }
  }
  function find(id: string) {
    select(id); setPanel(null)
    requestAnimationFrame(() => { const element = document.getElementById(`map-place-${id}`); element?.focus(); element?.scrollIntoView({ block: 'center', inline: 'center' }) })
  }
  return <div className="places-map-surface">
    <div className="places-map-viewport" ref={viewport} tabIndex={0} role="region" aria-label="Illustrated virtual map; drag or use arrow keys to pan" onKeyDown={panKey}
      onScroll={event => setScroll({ left: event.currentTarget.scrollLeft, top: event.currentTarget.scrollTop })}
      onPointerDown={event => { if (event.pointerType === 'mouse' && event.button === 0 && event.target === event.currentTarget.querySelector('img')) { drag.current = { x: event.clientX, y: event.clientY, left: event.currentTarget.scrollLeft, top: event.currentTarget.scrollTop }; event.currentTarget.setPointerCapture(event.pointerId) } }}
      onPointerMove={event => { const start = drag.current; if (start) { event.currentTarget.scrollLeft = start.left - event.clientX + start.x; event.currentTarget.scrollTop = start.top - event.clientY + start.y } }}
      onPointerUp={() => { drag.current = null }} onPointerCancel={() => { drag.current = null }} onLostPointerCapture={() => { drag.current = null }}>
      {!ready && <div className="places-map-fallback">{fallback}<p role="status">{failed === image.imageUrl ? 'Illustration unavailable. Schematic and place list remain available.' : 'Loading illustrated map…'}</p></div>}
      <div className="places-map-plane" style={{ width, height, visibility: ready ? 'visible' : 'hidden' }}>
        <img draggable={false} src={image.imageUrl} width={image.width} height={image.height} alt={`${image.themeId} illustrated virtual map`}
          onLoad={() => setLoaded(image.imageUrl)} onError={() => setFailed(image.imageUrl)} />
        {ready && <svg className="places-map-leaders" width={width} height={height} aria-hidden="true">{anchors.map(point => {
          const label = plaques.get(point.id)!
          return (label.x !== point.x || label.y !== point.y) && <line key={point.id} x1={point.x} y1={point.y} x2={label.x} y2={label.y} />
        })}</svg>}
        {ready && items.map(place => {
          const point = map.geometry.interactions.find(p => p.place_id === place.place_id)
          if (!point) return null
          const unavailable = state.unavailablePlaceIds.includes(place.place_id)
          const label = plaques.get(place.place_id)!
          return <ShellLink id={`map-place-${place.place_id}`} key={place.place_id} className="places-map-plaque" to={`/world/places/${place.place_id}`}
            style={{ left: label.x, top: label.y }} onPointerDown={() => select(place.place_id)}
            aria-label={`View ${place.label}${place.closure_state === 'CLOSED' ? ' — closed' : unavailable ? ' — unavailable' : ''}`} title={place.label}
            data-status={place.closure_state === 'CLOSED' ? 'closed' : unavailable ? 'unavailable' : 'open'} onFocus={() => select(place.place_id)}>
            <LandmarkIcon interaction={point.interaction_id} /><span>{place.label}</span>
            {place.closure_state === 'CLOSED' && <small>Closed</small>}{unavailable && <small>Unavailable</small>}
          </ShellLink>
        })}
        {ready && target && <span className="places-map-pin" role="img" aria-label={`Selected virtual location: ${current!.label}`}
          style={{ left: `${(target.cell_x + .5) / map.geometry.width_cells * 100}%`, top: `${(target.cell_y + .5) / map.geometry.height_cells * 100}%` }}>
          <svg viewBox="0 0 24 32" aria-hidden="true"><path d="M12 31S1 17 1 12a11 11 0 0 1 22 0c0 5-11 19-11 19Z" /><circle cx="12" cy="12" r="4" /></svg>
        </span>}
      </div>
    </div>
    <div className="places-map-zoom" aria-label="Map view zoom">
      <button type="button" aria-label="Zoom in map view" disabled={zoom >= 3 || !ready} onClick={() => setZoom(z => clamp(z + .25, minZoom, 3))}>+</button>
      <button type="button" aria-label="Zoom out map view" disabled={zoom <= minZoom || !ready} onClick={() => setZoom(z => clamp(z - .25, minZoom, 3))}>−</button>
    </div>
    <div className="places-map-caption" role="status">{current ? `${current.label} · ${supported ? 'virtual target selected' : physicalStatus(current)}` : 'Virtual map · select a landmark'}<span>No physical arrival is implied.</span></div>
    <div className="places-map-tools">
      <button type="button" onClick={event => { returnFocus.current = event.currentTarget; setPanel('places') }}><ConcertIcon name="world" />Places</button>
      <button type="button" onClick={event => { returnFocus.current = event.currentTarget; setPanel('legend') }}>Legend</button>
      <button type="button" disabled={!ready} onClick={() => { setZoom(minZoom); setReset(r => r + 1) }}>Fit map</button>
      <button type="button" disabled={!ready} onClick={() => { setZoom(1); selections.delete(controller); setSelected(null); setReset(r => r + 1) }}>Reset view</button>
    </div>
    <dialog ref={sheet} className="places-map-sheet" aria-labelledby="places-map-sheet-title"
      onKeyDown={event => { if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); setPanel(null) } }}
      onCancel={event => { event.preventDefault(); setPanel(null) }}>
      <header><h2 id="places-map-sheet-title">{panel === 'legend' ? 'Map legend' : 'Places'}</h2><button type="button" aria-label="Close map information" onClick={() => setPanel(null)}>×</button></header>
      {panel === 'legend' ? <>{legend}<p>Drag the map or focus it and use arrow keys. + and − change this overview only. Fit map shows the entire source composition.</p><p>Map revision {map.map_revision} · {image.themeId} revision {image.themeRevision}</p></> : <>
        <label className="places-map-search">Find a place<input type="search" value={search} maxLength={512} onChange={event => setSearch(event.target.value)} /></label>
        <ol>{items.filter(p => p.label.toLowerCase().includes(search.trim().toLowerCase())).map(place => <li key={place.place_id}>
          <button type="button" onClick={() => find(place.place_id)}>{place.label}</button><p>{state.unavailablePlaceIds.includes(place.place_id) ? 'Currently unavailable' : physicalStatus(place)}</p>
        </li>)}</ol>
        {!items.filter(p => p.label.toLowerCase().includes(search.trim().toLowerCase())).length && <p>No places match.</p>}
        <button type="button" onClick={() => void controller.refresh()}>Refresh place information</button>
        <ShellLink id="map-help" to="/world/help">Controls and meeting safety</ShellLink>
      </>}
    </dialog>
  </div>
}
