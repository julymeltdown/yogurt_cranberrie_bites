import { useEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import type { LandmarkId } from '../spatial/contract'
import type { LandmarkView, PersonView, PlaceNote, VenueUnavailableReason } from '../ports'
import { locationLabel, type Selection, type VenueMapController } from '../model/controller'
import { MapCanvas } from './MapCanvas'
import './paradise-city-map.css'

export function ParadiseCityMap({ controller, renderPortrait }: { readonly controller: VenueMapController
  readonly renderPortrait: (person: Pick<PersonView, 'displayName' | 'avatar'>, size: 64 | 128) => ReactNode }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const { venue } = controller, host = useRef<HTMLDivElement>(null), returnTo = useRef<HTMLElement | null>(null), wasOpen = useRef(false)
  const [consented, setConsented] = useState(false), [showSharing, setShowSharing] = useState(false), [confirmRevoke, setConfirmRevoke] = useState(false), [focusLandmark, setFocusLandmark] = useState<{ readonly id: LandmarkId; readonly serial: number } | null>(null)
  const [meetingPlace, setMeetingPlace] = useState<LandmarkId | ''>(''), [meetingTarget, setMeetingTarget] = useState('')
  const [notesPlace, setNotesPlace] = useState<LandmarkId | ''>('')
  const personId = state.selection?.kind === 'friend' ? state.selection.id : null
  const placeId = state.selection && 'id' in state.selection && state.selection.kind !== 'friend' ? state.selection.id : null
  const selectedPerson = personId ? state.people.find(p => p.userId === personId) : null
  const selectedPlace = placeId ? venue.landmarks.find(p => p.id === placeId) : null
  const selectedNotes = state.notes.filter(n => selectedPlace ? n.landmarkId === selectedPlace.id : true)
  const hasSheet = Boolean(state.selection || state.draft), locked = state.busy || state.uncertain || state.meeting?.locked
  const now = state.snapshot?.serverNowMs ?? null
  const availability = state.availability, friendsUnavailable = availability.state === 'unavailable', friendsStale = availability.state === 'stale'
  const snapshotAge = availability.ageMs === null ? 'unknown age' : availability.ageMs < 1000 ? 'just now'
    : availability.ageMs < 60000 ? `${Math.floor(availability.ageMs / 1000)}s ago` : `${Math.floor(availability.ageMs / 60000)}m ago`
  function unavailableText(reason: VenueUnavailableReason | null) {
    if (reason === 'friend-gate-unavailable') return 'The venue friendship authority is unavailable (503). Friend locations cannot be verified, so none are shown.'
    if (reason === 'offline') return 'This device is offline. Friend locations cannot be verified until the connection recovers.'
    if (reason === 'access-unavailable') return 'Current venue access has ended. Friend locations stay hidden until access is restored.'
    return 'Live location, check-in, meetings and notes are unavailable until current venue capabilities are supplied.'
  }
  const refreshFriends = () => { void controller.refreshAuthority() }
  const incoming = (state.snapshot?.locationRequests?.incoming ?? []).filter(r => r.subjectId === state.snapshot?.scope.userId && now !== null && r.expiresAtMs > now && state.people.some(p => p.userId === r.requesterId))
  const outgoing = (state.snapshot?.locationRequests?.outgoing ?? []).filter(r => r.requesterId === state.snapshot?.scope.userId && now !== null && r.expiresAtMs > now && state.people.some(p => p.userId === r.subjectId))
  useEffect(() => {
    if (hasSheet && !wasOpen.current) host.current?.querySelector<HTMLElement>('.pc-sheet button,.pc-sheet textarea')?.focus({ preventScroll: true })
    if (!hasSheet && wasOpen.current) (returnTo.current?.isConnected ? returnTo.current : host.current)?.focus({ preventScroll: true })
    wasOpen.current = hasSheet
  }, [hasSheet])
  useEffect(() => { if (!state.snapshot || state.snapshot.sharing === 'revoked') { setConsented(false); setShowSharing(false); setConfirmRevoke(false); setMeetingPlace(''); setMeetingTarget('') } }, [state.snapshot])
  useEffect(() => { if (state.snapshot?.sharing === 'active') setConfirmRevoke(false) }, [state.snapshot?.sharing])
  function select(value: Selection) { returnTo.current = document.activeElement instanceof HTMLElement ? document.activeElement : null; controller.select(value); setMeetingPlace(''); setMeetingTarget('') }
  function close() { controller.cancelDraft(); controller.select(null) }
  function placeLabel(person: PersonView) { const name = venue.landmarks.find(p => p.id === person.landmarkId)?.label; return name ? `${person.locationSource === 'last-known' ? 'Last known near' : 'Near'} ${name}` : 'Location unavailable' }
  function meetingButtons(person: Pick<PersonView, 'userId'>, place: LandmarkView | undefined, canMessage: boolean, canMeet: boolean, origin: 'map-friend' | 'map-landmark' | 'map-note') {
    return <div className="pc-actions"><button type="button" disabled={locked || !canMessage} onClick={() => { void controller.openMeeting('MESSAGE', person, place, origin) }}>MESSAGE</button>
      <button type="button" className="pc-primary" disabled={locked || !canMeet || !place?.meetingContext} onClick={() => { if (place) void controller.openMeeting('INVITE', person, place, origin) }}>MEET HERE</button></div>
  }
  function noteCard(note: PlaceNote) {
    const place = venue.landmarks.find(p => p.id === note.landmarkId), remaining = now === null ? null : Math.max(0, Math.ceil((note.expiresAtMs - now) / 60000))
    const age = now !== null && Number.isFinite(note.createdAtMs) && note.createdAtMs >= 0 && note.createdAtMs <= now ? Math.floor((now - note.createdAtMs) / 60000) : null
    return <article className="pc-note-card" key={note.id}>
      <div className="pc-note-heading"><div className="pc-portrait">{renderPortrait(note.author, 64)}</div><div><div className="pc-note-author"><strong className="pc-name">{note.author.displayName}</strong>{age !== null && <small>{age < 1 ? '<1m ago' : age >= 60 ? `${Math.floor(age / 60)}h ago` : `${age}m ago`}</small>}</div><p>{note.body}</p><small>{note.audience === 'FRIENDS' ? 'Friends' : 'Everyone'} · {remaining === null ? 'Expiry unavailable' : remaining >= 60 ? `expires in ${Math.ceil(remaining / 60)}h` : `expires in ${remaining}m`}</small></div></div>
      <div className="pc-actions"><button type="button" disabled={locked || !note.canReply} onClick={() => controller.compose(note.landmarkId, note.id)}>REPLY</button>
        <button type="button" className="pc-primary" disabled={locked || !note.canMeet || !place?.meetingContext} onClick={() => { if (place) void controller.openMeeting('INVITE', note.author, place, 'map-note') }}>MEET HERE</button></div>
      {note.replies.map(reply => <div className="pc-note-reply" key={reply.id}><div className="pc-portrait">{renderPortrait(reply.author, 64)}</div><div><strong>Reply to {note.author.displayName}</strong><p>{reply.body}</p></div></div>)}
      {controller.canLoadNoteThread && <button type="button" disabled={locked} onClick={() => { void controller.loadNotes(note.id, true) }}>READ REPLIES</button>}
    </article>
  }
  return <div className="pc-map" ref={host} tabIndex={-1} aria-label="Paradise City map" onKeyDown={event => {
    if (event.key === 'Escape' && !locked) { event.preventDefault(); if (state.meeting?.active) controller.closeMeeting(); else close() }
  }}>
    <div className="pc-map-tabs" role="tablist" aria-label="Venue map sections">{(['MAP', 'FRIENDS', 'NOTES'] as const).map((tab, index) => <button type="button" role="tab" key={tab} id={`pc-tab-${tab}`} aria-controls="pc-tab-panel" aria-selected={state.tab === tab} tabIndex={state.tab === tab ? 0 : -1} disabled={locked}
      onKeyDown={event => { if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) { event.preventDefault(); const tabs = ['MAP', 'FRIENDS', 'NOTES'] as const; const target = event.key === 'Home' ? 0 : event.key === 'End' ? 2 : (index + (event.key === 'ArrowRight' ? 1 : 2)) % 3; controller.setTab(tabs[target]!); document.getElementById(`pc-tab-${tabs[target]}`)?.focus() } }}
      onClick={() => controller.setTab(tab)}>{tab}</button>)}</div>
    {state.snapshot?.classification === 'CONTROLLED_FIXTURE' && <p className="pc-fixture-notice">SYNTHETIC EXAMPLE · positions, people and times are not live.</p>}
    {friendsUnavailable && state.tab !== 'FRIENDS' && <section className="pc-availability pc-availability--unavailable" role="status" aria-label="Venue friends unavailable">
      <strong>Friends unavailable</strong><p>{unavailableText(availability.reason)}</p>
      <button type="button" disabled={!availability.canRefresh || availability.refreshing} onClick={refreshFriends}>{availability.refreshing ? 'CHECKING…' : 'TRY AGAIN'}</button>
    </section>}
    {friendsStale && <section className="pc-availability pc-availability--stale" role="status" aria-label="Location snapshot status">
      <strong>Snapshot {snapshotAge}</strong><p>{availability.reason === 'offline' ? 'Offline. Showing last-known information, not current locations.' : 'Older than the venue freshness policy. Locations are shown as last-known.'}</p>
      <button type="button" disabled={!availability.canRefresh || availability.refreshing} onClick={refreshFriends}>{availability.refreshing ? 'CHECKING…' : 'REFRESH'}</button>
    </section>}
    <div id="pc-tab-panel" role="tabpanel" aria-labelledby={`pc-tab-${state.tab}`} className="pc-tab-panel">
      <MapCanvas venue={venue} people={state.people} self={controller.self()} notes={state.notes} selection={state.selection} onSelect={select} renderPortrait={renderPortrait} focusLandmark={state.focusRequest ?? focusLandmark}
        compact={state.tab !== 'MAP' || hasSheet || Boolean(state.meeting?.active)} />
      {state.meeting?.active ? <div className="pc-meeting-slot">{state.meeting.content}</div> : state.draft ? <section className="pc-sheet pc-composer" aria-label={state.draft.replyTo ? 'Reply to place note' : 'Leave a note'}>
        <header><h2>{state.draft.replyTo ? 'Reply to note' : 'Leave a note'}</h2><button type="button" aria-label="Close note composer" disabled={locked} onClick={close}>×</button></header>
        <label className="pc-place-picker">Place<select aria-label="Note place" disabled={locked || Boolean(state.draft.replyTo)} value={state.draft.landmarkId} onChange={event => controller.changeDraftPlace(event.target.value)}>
          {venue.landmarks.filter(p => state.snapshot?.rights.postNote.includes(p.id) || p.id === state.draft?.landmarkId).map(p => <option value={p.id} key={p.id}>{p.label}</option>)}</select></label>
        <label><span className="pc-sr-only">{state.draft.replyTo ? 'Reply text' : 'Note text'}</span><textarea autoComplete="off" maxLength={1000} rows={4} value={state.draft.body} disabled={locked} placeholder="Write a message for this place…" onChange={event => controller.edit(event.target.value)} /></label>
        {!state.draft.replyTo && <><fieldset disabled={locked}><legend>Visible to</legend>{(['FRIENDS', 'EVERYONE'] as const).map(value => <button type="button" key={value} aria-pressed={state.draft?.audience === value} disabled={!state.snapshot?.rights.noteAudiences.includes(value)} onClick={() => controller.setAudience(value)}>{value}</button>)}</fieldset>
          <fieldset disabled={locked}><legend>Expires after</legend>{(['ONE_HOUR', 'END_OF_DAY'] as const).map(value => <button type="button" key={value} aria-pressed={state.draft?.expiry === value} disabled={!state.snapshot?.rights.noteExpiries.includes(value)} onClick={() => controller.setExpiry(value)}>{value === 'ONE_HOUR' ? '1 HOUR' : 'END OF DAY'}</button>)}</fieldset></>}
        <button type="button" className="pc-primary pc-full" disabled={locked || !state.draft.body.trim()} onClick={() => { void controller.submit() }}>{state.draft.replyTo ? 'POST REPLY' : '+ POST NOTE'}</button>
      </section> : selectedPerson ? <section className="pc-sheet" role="dialog" aria-modal="false" aria-label={selectedPerson.displayName}>
        <header><h2 className="pc-sr-only">{selectedPerson.displayName}</h2><button type="button" className="pc-close" aria-label="Close friend details" disabled={locked} onClick={close}>×</button></header>
        <div className="pc-friend-detail"><div className="pc-portrait pc-portrait--large">{renderPortrait(selectedPerson, 128)}</div><div><h2>{selectedPerson.displayName}</h2><p>⌖ {placeLabel(selectedPerson)}</p><small>{locationLabel(selectedPerson, now)}</small></div></div>
        <label className="pc-place-picker">Meeting place<select aria-label="Friend meeting place" disabled={locked} value={meetingPlace || selectedPerson.landmarkId || ''} onChange={event => setMeetingPlace(venue.landmarks.find(p => p.id === event.target.value)?.id ?? '')}><option value="">Choose a fixed landmark</option>{venue.landmarks.filter(p => p.meetingContext !== null).map(p => <option key={p.id} value={p.id}>{p.label}</option>)}</select></label>
        {meetingButtons(selectedPerson, venue.landmarks.find(p => p.id === (meetingPlace || selectedPerson.landmarkId)), selectedPerson.canMessage, selectedPerson.canMeet, 'map-friend')}
        {selectedPerson.canRequest && <button type="button" className="pc-full" disabled={locked} onClick={() => { void controller.requestLocation(selectedPerson.userId) }}>ASK LOCATION</button>}
      </section> : state.selection?.kind === 'cluster' ? <section className="pc-sheet" aria-label="Friend cluster"><header><h2>{state.selection.ids.length} friends</h2><button type="button" aria-label="Close friend cluster" onClick={close}>×</button></header>
        {state.people.filter(p => state.selection?.kind === 'cluster' && state.selection.ids.includes(p.userId)).map(p => <button type="button" className="pc-cluster-person" key={p.userId} onClick={() => select({ kind: 'friend', id: p.userId })}>{renderPortrait(p, 64)}<strong>{p.displayName}</strong></button>)}
      </section> : selectedPlace && state.selection?.kind === 'landmark' ? <section className="pc-sheet" aria-label={`${selectedPlace.label} location card`}><header><h2>⌖ {selectedPlace.label}</h2><button type="button" aria-label="Close location card" onClick={close}>×</button></header>
        <p>Fixed landmark on the illustrated Paradise City siteplan.</p><p className="pc-meta">{selectedPlace.meetingContext !== null ? 'Current meeting context supplied by the venue adapter.' : 'Illustrated reference · real-place approval unavailable.'}</p>
        <p className="pc-meta" data-landmark-review={selectedPlace.illustration?.review ?? 'UNAVAILABLE'} data-landmark-sources={selectedPlace.illustration?.sourceRefs.map(source => source.id).join(',') ?? ''}>{selectedPlace.illustration ? `Illustration source: ${selectedPlace.illustration.sourceRefs.map(source => source.id.replace(/^photo-/, 'Photo ')).join(' · ')}. Uncalibrated illustration; physical placement unverified.` : 'Illustration source and review unavailable.'}</p>
        <div className="pc-actions"><button type="button" onClick={() => setFocusLandmark(value => ({ id: selectedPlace.id, serial: (value?.serial ?? 0) + 1 }))}>View on map</button><button type="button" disabled={locked || !state.snapshot?.rights.checkIn.includes(selectedPlace.id)} onClick={() => { void controller.checkIn(selectedPlace.id) }}>CHECK IN</button></div>
        <label>Meet with<select aria-label="Meeting friend" value={meetingTarget} onChange={event => setMeetingTarget(event.target.value)}><option value="">Choose a friend</option>{state.people.filter(p => p.canMeet || p.canMessage).map(p => <option key={p.userId} value={p.userId}>{p.displayName}</option>)}</select></label>
        {meetingButtons({ userId: meetingTarget }, selectedPlace, Boolean(state.people.find(p => p.userId === meetingTarget)?.canMessage), Boolean(state.people.find(p => p.userId === meetingTarget)?.canMeet), 'map-landmark')}
        <button type="button" className="pc-full" disabled={locked || !state.snapshot?.rights.postNote.includes(selectedPlace.id)} onClick={() => controller.compose(selectedPlace.id)}>+ LEAVE A NOTE</button>
      </section> : state.tab === 'FRIENDS' ? <div className="pc-friends" aria-label="Friends at the venue">{friendsUnavailable ? <section className="pc-unavailable" role="status" aria-label="Friends unavailable">
        <h2>Friends unavailable</h2><p>{unavailableText(availability.reason)}</p>
        <button type="button" disabled={!availability.canRefresh || availability.refreshing} onClick={refreshFriends}>{availability.refreshing ? 'CHECKING…' : 'TRY AGAIN'}</button>
      </section> : state.people.length ? state.people.map(person => <article className="pc-friend-row" key={person.userId}>
        <div className="pc-portrait">{renderPortrait(person, 64)}</div><div className="pc-friend-copy"><button type="button" className="pc-name-button" aria-label={person.displayName} onClick={() => select({ kind: 'friend', id: person.userId })}><strong className="pc-name">{person.displayName}</strong><span className="pc-friend-place">⌖ {placeLabel(person)}</span><small>{locationLabel(person, now)}</small></button></div>
        <button type="button" disabled={locked} onClick={() => { if (person.canRequest && (person.locationSource === 'last-known' || person.locationSource === 'unavailable')) void controller.requestLocation(person.userId); else select({ kind: 'friend', id: person.userId }) }}>{person.canRequest && (person.locationSource === 'last-known' || person.locationSource === 'unavailable') ? 'ASK LOCATION' : 'VIEW'}</button>
      </article>) : <p role="status">No friend location is currently authorized. A friendship alone does not share location.</p>}
      {(incoming.length > 0 || outgoing.length > 0) && <section className="pc-location-requests" aria-label="Location requests"><h2>Location requests</h2>
        {incoming.map(request => <article key={request.id}><p>{state.people.find(p => p.userId === request.requesterId)?.displayName} asked for your location.</p><div className="pc-actions">
          <button type="button" disabled={locked} onClick={() => { void controller.decideLocationRequest(request.id, 'APPROVE') }}>APPROVE</button>
          <button type="button" disabled={locked} onClick={() => { void controller.decideLocationRequest(request.id, 'DENY') }}>DENY</button></div></article>)}
        {outgoing.map(request => <article key={request.id}><p>Waiting for {state.people.find(p => p.userId === request.subjectId)?.displayName} to respond.</p><button type="button" disabled={locked} onClick={() => { void controller.decideLocationRequest(request.id, 'CANCEL') }}>CANCEL REQUEST</button></article>)}
      </section>}</div> : state.tab === 'NOTES' || state.selection?.kind === 'notes' ? <section className="pc-sheet pc-notes" aria-label="Place notes">
        {controller.canLoadNotes && <><label className="pc-place-picker">Read notes at<select aria-label="Read notes at place" value={notesPlace || selectedPlace?.id || venue.landmarks[0]?.id || ''} disabled={locked} onChange={event => setNotesPlace(venue.landmarks.find(p => p.id === event.target.value)?.id ?? '')}>{venue.landmarks.map(p => <option key={p.id} value={p.id}>{p.label}</option>)}</select></label>
        <button type="button" disabled={locked} onClick={() => { const id = notesPlace || selectedPlace?.id || venue.landmarks[0]?.id; if (id) void controller.loadNotes(id) }}>READ NOTES</button></>}
        {selectedPlace && <header><h2>{selectedPlace.label.toUpperCase()} / {selectedNotes.length} NOTES</h2><button type="button" aria-label="Close selected notes" disabled={locked} onClick={close}>×</button></header>}
        {selectedNotes.length ? selectedNotes.map(noteCard) : <p role="status">No current notes are available for this place.</p>}
        <button type="button" className="pc-primary pc-full" disabled={locked || !state.snapshot?.rights.postNote.length} onClick={() => { const id = selectedPlace?.id ?? state.snapshot?.rights.postNote[0]; if (id) controller.compose(id) }}>+ NEW NOTE</button>
      </section> : null}
    </div>
    <div className="pc-sharing"><strong>{state.snapshot?.sharing === 'active' ? '♟ Sharing with friends' : state.snapshot?.sharing === 'paused' ? 'Location sharing paused' : state.snapshot?.sharing === 'revoked' ? 'Location sharing revoked' : 'Phone location is off'}</strong>
      <div className="pc-sharing-actions">{state.snapshot?.sharing === 'active'
        ? <><button type="button" onClick={() => { void controller.pause() }}>PAUSE</button>
          <button type="button" className="pc-danger" disabled={!state.snapshot?.rights.revoke || locked} onClick={() => setConfirmRevoke(true)}>REVOKE</button></>
        : <button type="button" disabled={!state.snapshot?.rights.sharePhone || locked} onClick={() => setShowSharing(value => !value)}>SHARE LOCATION</button>}</div></div>
    {showSharing && <section className="pc-sharing-consent" aria-label="Phone location sharing"><p>Share real phone location with authorized friends. This is separate from your virtual World position and from device permission. Your browser asks for permission only after you continue.</p><label><input type="checkbox" checked={consented} onChange={event => setConsented(event.target.checked)} /> I want to share my phone location.</label>
      <button type="button" className="pc-primary" disabled={!consented || locked} onClick={() => { void controller.startPhone(consented); setShowSharing(false) }}>Allow phone location</button></section>}
    {confirmRevoke && <section className="pc-sharing-consent" role="alertdialog" aria-label="Stop sharing location"><p>Revoke location sharing? Your selected audience and any current phone or check-in location are removed immediately.</p>
      <div className="pc-actions"><button type="button" onClick={() => setConfirmRevoke(false)}>KEEP SHARING</button><button type="button" className="pc-danger" disabled={locked} onClick={() => { setConfirmRevoke(false); void controller.revoke() }}>STOP SHARING</button></div></section>}
    {state.sharingFailure && <section className="pc-permission" role="status" aria-label="Phone location unavailable"><strong>{state.sharingFailure === 'permission-denied' ? 'Phone permission denied' : state.sharingFailure === 'server-refused' ? 'Sharing not authorized' : 'Phone location unavailable'}</strong>
      <p>{state.sharingFailure === 'permission-denied' ? 'No location is being requested or shared. Enable location for this site in browser settings, then review sharing again.' : state.sharingFailure === 'server-refused' ? 'The server did not authorize phone sharing. Nothing is being shared; review the audience before trying again.' : 'This browser or device cannot provide a phone location. Check-in remains available only where the venue permits it.'}</p>
      {state.sharingFailure !== 'device-unavailable' && <button type="button" disabled={locked} onClick={() => setShowSharing(true)}>REVIEW SHARING</button>}</section>}
    {state.notice && <p role="status" className="pc-notice">{state.notice}</p>}
    {state.uncertain && <button type="button" disabled={state.busy} onClick={() => { void controller.resolve() }}>Check action status</button>}
  </div>
}
