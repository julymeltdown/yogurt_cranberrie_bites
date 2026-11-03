import { useEffect, useId, useState, useSyncExternalStore } from 'react'
import { ShellLink } from '../../../app/navigation'
import { Feedback, Field } from '../../../shared/ui'
import { ConcertAction, ConcertWindow } from '../../../shared/ui/concert-window'
import { physicalStatus, sourceLink, type Place } from '../model/contract'
import type { PlaceFailure, PlacesController, PlacesState } from '../model/controller'
import './places.css'

function usePlaces(controller: PlacesController) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  useEffect(() => { controller.start() }, [controller])
  return state
}
const failures: Record<PlaceFailure, { title: string; text: string }> = {
  'signed-out': { title: 'Sign in to see places', text: 'Your account or event changed. Return to the world to check access.' },
  denied: { title: 'Places are not available to this account', text: 'The server did not permit this read. No place details have been opened.' },
  missing: { title: 'This place is unavailable', text: 'It may have been removed or is no longer shared. Return to the map for the current list.' },
  conflict: { title: 'Place information changed', text: 'The list and detail use different revisions. Reload places before using a map target.' },
  'map-mismatch': { title: 'Your map and places do not match', text: 'Map targets are paused. Reload the event map and places together before continuing.' },
  'rate-limited': { title: 'Please wait before retrying', text: 'The service is receiving too many requests. Wait a moment, then try again.' },
  invalid: { title: 'Place information could not be checked', text: 'The response did not match this event or the place contract. No map target has been used.' },
  unavailable: { title: 'Places could not be loaded', text: 'The service may be unavailable. Your avatar and physical location have not changed.' },
}
function Failure({ reason, controller }: { reason: PlaceFailure; controller: PlacesController }) {
  const message = failures[reason]
  return <Feedback kind="error" title={message.title}><p>{message.text}</p>
    <div className="places-actions">
      {reason === 'map-mismatch' ? <ConcertAction onClick={() => window.location.reload()}>Reload event</ConcertAction>
        : !['signed-out', 'denied'].includes(reason) && <ConcertAction onClick={() => void controller.refresh()}>Reload places</ConcertAction>}
      <ShellLink id="places-recovery-world" className="places-link" to="/world">Return to world</ShellLink>
    </div>
  </Feedback>
}
export function PlaceLegend() {
  return <section className="places-legend" aria-label="Virtual and physical place legend">
    <p><strong>Virtual concept</strong> An in-world landmark. No real meeting destination.</p>
    <p><strong>Physical reference</strong> Venue information with a source and review state. Check closures and current guidance.</p>
    <p>Moving your avatar or camera never confirms physical arrival. No GPS, travel time or safe route is inferred.</p>
  </section>
}
function OverviewDiagram({ state }: { state: PlacesState }) {
  const id = useId(), map = state.map
  if (!map || state.mapMismatch) return null
  const g = map.geometry, tiles: string[] = []
  for (let y = 0; y < g.height_cells; y++) for (let x = 0; x < g.width_cells; x++) {
    if (g.collision_grid[y]?.[x] === 0) tiles.push(`M${x},${y}h1v1h-1z`)
  }
  return <figure className="places-diagram">
    <svg viewBox={`0 0 ${g.width_cells} ${g.height_cells}`} role="img" aria-labelledby={`${id}-title ${id}-description`}>
      <title id={`${id}-title`}>Virtual map overview</title>
      <desc id={`${id}-description`}>Schematic walkable cells and numbered landmarks. Use the place list below for details. This is not a physical venue route.</desc>
      <rect width={g.width_cells} height={g.height_cells} className="places-blocked" />
      <path d={tiles.join('')} className="places-walkable" />
      {state.list.data?.items.map((place, index) => {
        const point = g.interactions.find(p => p.place_id === place.place_id)
        return point && <g key={place.place_id} aria-hidden="true">
          <circle cx={point.cell_x + .5} cy={point.cell_y + .5} r=".42" />
          <text x={point.cell_x + .5} y={point.cell_y + .5} textAnchor="middle" dominantBaseline="central">{place.closure_state === 'CLOSED' ? '×' : index + 1}</text>
        </g>
      })}
    </svg>
    <figcaption>Schema-derived virtual schematic · map revision {map.map_revision}. Numbers match the place list; × means closed. Not venue artwork or a physical route.</figcaption>
  </figure>
}
export function PlacesOverview({ controller }: { controller: PlacesController }) {
  const state = usePlaces(controller), [search, setSearch] = useState('')
  useEffect(() => { controller.select(null) }, [controller])
  const items = state.list.data?.items ?? [], shown = items.map((place, index) => ({ place, index }))
    .filter(({ place }) => place.label.toLowerCase().includes(search.trim().toLowerCase()))
  return <section className="places-view places-overview" aria-label="Map overview"><ConcertWindow title="MAP" icon="world">
    {['idle', 'loading'].includes(state.list.phase) && <p role="status">Loading places…</p>}
    {state.list.phase === 'error' && <Failure reason={state.list.reason!} controller={controller} />}
    {['ready', 'empty'].includes(state.list.phase) && <>
      {state.mapMismatch && <Failure reason="map-mismatch" controller={controller} />}
      <OverviewDiagram state={state} />
      {!state.mapMismatch && <div className="places-actions" aria-label="Virtual camera controls">
        <ConcertAction tone="paper" disabled={!controller.cameraAvailable()} onClick={() => controller.recenter()}>Recenter avatar</ConcertAction>
        <ConcertAction tone="paper" disabled={!controller.cameraAvailable()} onClick={() => controller.zoom(-1)}>Zoom out</ConcertAction>
        <ConcertAction tone="paper" disabled={!controller.cameraAvailable()} onClick={() => controller.zoom(1)}>Zoom in</ConcertAction>
      </div>}
      <p className="places-camera-notice" role="status">{state.notice}</p>
      {!controller.cameraAvailable() && !state.mapMismatch && <p>Live camera controls are unavailable. All listed places can still be read.</p>}
      <p className="places-virtual-note">Virtual movement never confirms real arrival.</p>
      <details className="places-legend-disclosure"><summary>Virtual and physical place legend</summary><PlaceLegend /></details>
      {state.list.phase === 'empty' ? <p role="status">No places are published for this event yet.</p> : <>
        <Field label="Find a place" type="search" value={search} maxLength={512} onChange={e => setSearch(e.target.value)} />
        <p role="status">{shown.length} {shown.length === 1 ? 'place' : 'places'} shown</p>
        <ol className="places-list">
          {shown.map(({ place, index }) => <li key={place.place_id}>
            <p className="places-number">{String(index + 1).padStart(2, '0')}</p>
            <div><h3>{place.label}</h3><p>{state.unavailablePlaceIds.includes(place.place_id)
              ? 'Currently unavailable — open details to check again' : physicalStatus(place)}</p>
              <div className="places-actions">
                <ShellLink id={`place-${place.place_id}`} className="places-link" to={`/world/places/${place.place_id}`}>View {place.label}</ShellLink>
                <ConcertAction tone="paper" disabled={!controller.canShow(place.place_id)} onClick={() => controller.showOnMap(place.place_id)} aria-label={`Show ${place.label} on virtual map`}>Show on map</ConcertAction>
              </div>
            </div>
          </li>)}
        </ol>
        {!shown.length && <p>No places match. Try a different name.</p>}
      </>}
      <ConcertAction tone="paper" onClick={() => void controller.refresh()}>Refresh place information</ConcertAction>
    </>}
    <ShellLink id="places-help" className="places-link" to="/world/help">Controls and meeting safety</ShellLink>
  </ConcertWindow></section>
}
function Provenance({ place }: { place: Place }) {
  const p = place.provenance, href = sourceLink(p.source_uri)
  return <section aria-labelledby="place-source-heading"><h3 id="place-source-heading">Source and review</h3>
    <dl className="places-facts">
      <div><dt>Reference</dt><dd>{p.source_id} · {p.source_revision}</dd></div>
      <div><dt>Review</dt><dd>{p.review_state === 'REVIEWED' ? 'Reviewed' : p.review_state === 'REJECTED' ? 'Rejected' : 'Not reviewed'}</dd></div>
      {p.reviewed_by && <div><dt>Reviewed by</dt><dd>{p.reviewed_by}</dd></div>}
      {p.reviewed_at && <div><dt>Review date</dt><dd><time dateTime={p.reviewed_at}>{new Date(p.reviewed_at).toISOString()}</time></dd></div>}
      <div><dt>Author</dt><dd>{p.author}</dd></div>
      <div><dt>Use permission</dt><dd>{p.license}</dd></div>
      <div><dt>Photo permission</dt><dd>{{ NOT_APPLICABLE: 'Not applicable', UNREVIEWED: 'Not reviewed', GRANTED: 'Granted', DENIED: 'Denied' }[p.photo_permission]}</dd></div>
    </dl>
    {href ? <a className="places-link" href={href} target="_blank" rel="noopener noreferrer" referrerPolicy="no-referrer">Open source reference (new tab)</a>
      : <p>Source link unavailable. No external destination is offered.</p>}
    <p>No authorized place photo is available in this place information.</p>
  </section>
}
export function PlaceDetail({ controller, placeId }: { controller: PlacesController; placeId: string }) {
  const state = usePlaces(controller)
  useEffect(() => { controller.select(placeId); return () => { if (controller.read().selectedId === placeId) controller.select(null) } }, [controller, placeId])
  const detail = state.selectedId === placeId ? state.detail : null, place = detail?.data?.place
  return <section className="places-view places-detail" aria-label="Place information"><ConcertWindow title={place?.label ?? 'PLACE'} icon="world">
    <ShellLink id="place-back-map" className="places-link" to="/world/map">All places</ShellLink>
    {state.list.phase === 'error' ? <Failure reason={state.list.reason!} controller={controller} />
      : detail?.phase === 'error' ? <Failure reason={detail.reason!} controller={controller} />
        : !place ? <p role="status">Loading place information…</p> : <>
          <header className="places-intro"><p className="places-kicker">Place information · revision {detail!.data!.revision}</p>
            <p className="places-status">{physicalStatus(place)}</p></header>
          {state.mapMismatch && <Failure reason="map-mismatch" controller={controller} />}
          <section><h3>Before you go</h3><p>{place.accessibility_notes}</p>
            <p>{place.place_kind === 'VIRTUAL_CONCEPT' ? 'This landmark exists in the virtual world only. Do not use it as a physical meeting destination.'
              : place.closure_state === 'CLOSED' ? 'This place is closed. Choose another open, reviewed reference for a physical meeting.'
                : place.verification_state !== 'VERIFIED' ? 'Physical details are unverified. Do not rely on this place as a verified meeting destination.'
                  : 'A reviewed reference is not a live safety or access guarantee. Follow current venue instructions and check closures.'}</p>
          </section>
          <Provenance place={place} />
          <div className="places-actions"><ConcertAction disabled={!controller.canShow(placeId)} onClick={() => controller.showOnMap(placeId)}>Show on virtual map</ConcertAction>
            <ConcertAction tone="paper" onClick={() => void controller.refresh()}>Refresh place information</ConcertAction></div>
          {!controller.canShow(placeId) && <p>Virtual map targeting is unavailable for this place or release.</p>}
          <p role="status">{state.notice}</p>
          <PlaceLegend />
        </>}
    <ShellLink id="place-detail-help" className="places-link" to="/world/help">Read controls and meeting safety</ShellLink>
  </ConcertWindow></section>
}
