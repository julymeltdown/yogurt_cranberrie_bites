import { OperationsSurface, OperationsFrame } from '../parade-ink-operations/OperationsSurface'
import { useEffect, useRef, useState } from 'react'
import type { OperationsController } from '../event/model/controller'
import { Field, OperationFeedback, OperationsGate, useOperations, useReviewFocus } from '../event/ui/common'
import type { PlaceAdmin } from './contract'

export function PlacesOperations({ controller }: { controller: OperationsController }) {
  const state = useOperations(controller), [selected, setSelected] = useState<string | null>(null), review = useRef<HTMLDivElement>(null)
  const row = state.places.find(row => row.place.place_id === selected)
  useEffect(() => { void controller.loadPlaces() }, [controller])
  useEffect(() => { if (state.access !== 'ready') setSelected(null) }, [state.access])
  useReviewFocus(state, review)
  return <OperationsSurface view="places"><OperationsGate state={state}><OperationsFrame title="Venue and places">
    <p>Concept art never verifies a physical venue. Changes require the current place revision and an independently qualified map/write release.</p>
    <p>Write map revision: {controller.mapRevision() ?? 'Unavailable'}. The place revision below is separate from a catalog/map revision.</p>
    <button type="button" disabled={state.loading || state.command?.stage === 'review' || state.command?.stage === 'sending'} onClick={() => { void controller.loadPlaces() }}>Refresh places</button>
    <div ref={review}><OperationFeedback state={state} controller={controller} /></div>
    {state.places.length === 0 && !state.loading && <p>No places in this authorized page.</p>}
    <div className="ops-table-wrap" role="region" aria-label="Scrollable place review" tabIndex={0}><table><caption>Current place review and closure</caption><thead><tr><th scope="col">Place</th><th scope="col">State</th><th scope="col">Review</th></tr></thead>
      <tbody>{state.places.map(row => <tr key={row.place.place_id}><th scope="row">{row.place.label}<br />Revision {row.revision}</th>
        <td className="ops-state">{row.place.closure_state}<br />{row.place.verification_state}</td><td><button type="button" disabled={state.command !== null || state.loading} onClick={() => setSelected(row.place.place_id)}>Review {row.place.label}</button></td></tr>)}</tbody></table></div>
    {state.placeCursor && <button type="button" disabled={state.loading || state.command !== null} onClick={() => { setSelected(null); void controller.loadPlaces(true) }}>Next places page</button>}
    {row && <PlaceForm key={row.place.place_id} row={row} controller={controller} locked={state.command !== null || state.loading} onClose={() => setSelected(null)} />}
  </OperationsFrame></OperationsGate></OperationsSurface>
}
function PlaceForm({ row, controller, locked, onClose }: { row: PlaceAdmin; controller: OperationsController; locked: boolean; onClose: () => void }) {
  const [place, setPlace] = useState(row.place), [reason, setReason] = useState('')
  const heading = useRef<HTMLHeadingElement>(null)
  useEffect(() => { const opener = document.activeElement; heading.current?.focus(); return () => { if (opener instanceof HTMLElement && opener.isConnected) opener.focus() } }, [])
  return <section><h2 ref={heading} tabIndex={-1}>Review place source</h2><dl><dt>Place</dt><dd>{row.place.place_id}</dd><dt>Current revision</dt><dd>{row.revision}</dd>
    <dt>Current source</dt><dd>{row.place.provenance.source_uri ?? 'No physical source'}</dd><dt>Source revision</dt><dd>{row.place.provenance.source_revision}</dd>
    <dt>Review state</dt><dd>{row.place.provenance.review_state}</dd><dt>Reviewed by</dt><dd>{row.place.provenance.reviewed_by ?? 'Not reviewed'}</dd>
    <dt>Reviewed at</dt><dd>{row.place.provenance.reviewed_at ?? 'Not reviewed'}</dd><dt>Source kind</dt><dd>{row.place.provenance.source_kind}</dd>
    <dt>Author</dt><dd>{row.place.provenance.author}</dd><dt>License</dt><dd>{row.place.provenance.license}</dd><dt>Photo permission</dt><dd>{row.place.provenance.photo_permission}</dd></dl>
    <p>Closure-only changes preserve immutable provenance. Source, verification, accessibility or other edits must exactly match a separately reviewed new release. Draft inputs survive refresh; compare the current revision.</p>
    <form onSubmit={e => { e.preventDefault(); controller.preparePlace({ expected_revision: row.revision, place, reason }) }}><fieldset disabled={locked}><legend>Proposed place change</legend>
      <Field label="Place label">{id => <input id={id} required maxLength={512} value={place.label} onChange={e => setPlace({ ...place, label: e.target.value })} />}</Field>
      <Field label="Verification state">{id => <select id={id} value={place.verification_state} onChange={e => setPlace({ ...place, verification_state: e.target.value as typeof place.verification_state })}>
        {place.place_kind === 'VIRTUAL_CONCEPT' ? <option>CONCEPT_ONLY</option> : <><option>UNVERIFIED</option><option>VERIFIED</option></>}</select>}</Field>
      <Field label="Closure state">{id => <select id={id} value={place.closure_state} onChange={e => setPlace({ ...place, closure_state: e.target.value as typeof place.closure_state })}><option>OPEN</option><option>CLOSED</option></select>}</Field>
      <Field label="Accessibility notes">{id => <textarea id={id} required maxLength={512} value={place.accessibility_notes} onChange={e => setPlace({ ...place, accessibility_notes: e.target.value })} />}</Field>
      <Field label="Place source URI">{id => <input id={id} type="url" value={place.provenance.source_uri ?? ''} onChange={e => setPlace({ ...place, provenance: { ...place.provenance, source_uri: e.target.value || null } })} />}</Field>
      <Field label="Source revision">{id => <input id={id} required maxLength={512} value={place.provenance.source_revision} onChange={e => setPlace({ ...place, provenance: { ...place.provenance, source_revision: e.target.value } })} />}</Field>
      <Field label="Source review state">{id => <select id={id} value={place.provenance.review_state} onChange={e => setPlace({ ...place, provenance: { ...place.provenance,
        review_state: e.target.value as typeof place.provenance.review_state, reviewed_by: null, reviewed_at: null } })}><option>UNREVIEWED</option><option>REVIEWED</option><option>REJECTED</option></select>}</Field>
      {place.provenance.review_state === 'REVIEWED' && <><Field label="Source reviewed by">{id => <input id={id} required maxLength={512} value={place.provenance.reviewed_by ?? ''} onChange={e => setPlace({ ...place, provenance: { ...place.provenance, reviewed_by: e.target.value } })} />}</Field>
        <Field label="Source reviewed at (UTC)">{id => <input id={id} required placeholder="YYYY-MM-DDTHH:mm:ssZ" value={place.provenance.reviewed_at ?? ''} onChange={e => setPlace({ ...place, provenance: { ...place.provenance, reviewed_at: e.target.value } })} />}</Field></>}
      <Field label="Reason for place change">{id => <textarea id={id} required maxLength={500} value={reason} onChange={e => setReason(e.target.value)} />}</Field>
      <div className="ops-actions"><button type="submit">Review place change</button><button type="button" onClick={onClose}>Close place editor</button></div>
    </fieldset></form>
  </section>
}
