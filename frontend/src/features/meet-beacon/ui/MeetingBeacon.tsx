import { useEffect, useId, useState, useSyncExternalStore } from 'react'
import { sourceLink } from '../../places/model/contract'
import { meetingNotice, type MeetingBeaconController } from '../model/controller'
import './meeting-beacon.css'

function useMeeting(controller: MeetingBeaconController) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  useEffect(() => { controller.start() }, [controller])
  return state
}
function CommandStatus({ controller }: { controller: MeetingBeaconController }) {
  const state = useMeeting(controller), phase = state.command.phase
  return <div aria-live="polite">
    {state.notice && <p role="alert">{meetingNotice[state.notice]}</p>}
    {phase === 'sending' && <p>Sending once. Wait for the server result.</p>}
    {['uncertain', 'pending', 'checking', 'retired'].includes(phase) && <><p>{phase === 'retired'
      ? 'The command can no longer be reconciled here. Ask for current authorized information before taking further action.'
      : 'The command outcome is unconfirmed. Check its status before making another change.'}</p>
      <button type="button" disabled={phase === 'checking' || phase === 'retired' || !state.canRead} onClick={() => { void controller.checkCommand() }}>Check command status</button></>}
    {phase === 'succeeded' && <p>Change confirmed by the server.</p>}
  </div>
}
export function MeetingBeaconForm({ controller, mode = 'create' }: { readonly controller: MeetingBeaconController; readonly mode?: 'create' | 'update' }) {
  const state = useMeeting(controller), id = useId(), d = state.draft
  const blocked = !state.canWrite || ['sending', 'uncertain', 'pending', 'checking', 'retired'].includes(state.command.phase)
  return <section className="meeting-beacon" aria-labelledby={`${id}-title`}>
    <h2 id={`${id}-title`}>{mode === 'create' ? 'Real-world check-in' : 'Update meeting guidance'}</h2>
    <p>Choose a reviewed physical place and who may see your description. Virtual movement never shares a location.</p>
    {!state.canWrite && <p role="status">Current place, audience, time, and write authority are required. Meeting changes are unavailable.</p>}
    <form onSubmit={event => { event.preventDefault(); void (mode === 'create' ? controller.create() : controller.update()) }}>
      <fieldset disabled={blocked}>
        <legend>Private meeting description</legend>
        <label htmlFor={`${id}-place`}>Reviewed place</label>
        <select id={`${id}-place`} required value={d.placeId} onChange={e => controller.setDraft({ placeId: e.target.value, acknowledged: false })}>
          <option value="">Choose a place</option>{state.places.map(p => <option value={p.id} key={p.id}>{p.label}</option>)}
        </select>
        {!state.places.length && <p>No currently qualified physical places are available.</p>}
        <label htmlFor={`${id}-audience`}>Who can see this?</label>
        <select id={`${id}-audience`} required value={d.audienceId} onChange={e => controller.setDraft({ audienceId: e.target.value, acknowledged: false })}>
          <option value="">Choose an authorized audience</option>{state.audiences.map(a => <option value={a.id} key={a.id}>{a.label}</option>)}
        </select>
        <label htmlFor={`${id}-instructions`}>How to find me</label>
        <textarea id={`${id}-instructions`} value={d.instructions} required maxLength={1000} rows={3}
          onChange={e => controller.setDraft({ instructions: e.target.value, acknowledged: false })} />
        <label htmlFor={`${id}-identification`}>Identification</label>
        <input id={`${id}-identification`} value={d.identification} required maxLength={240}
          onChange={e => controller.setDraft({ identification: e.target.value, acknowledged: false })} />
        <p>Use an identifying detail you choose to share. Photo attachment is unavailable.</p>
        <label htmlFor={`${id}-ttl`}>Visible for (seconds)</label>
        <input id={`${id}-ttl`} type="number" min={1} max={state.maxTtlSeconds || 900} step={1} required value={d.ttlSeconds}
          onChange={e => controller.setDraft({ ttlSeconds: e.target.valueAsNumber, acknowledged: false })} />
        <p>Up to {state.maxTtlSeconds || 900} seconds, ending earlier if the event closes. Initial default: 15 minutes.</p>
        <label className="meeting-check"><input type="checkbox" checked={d.acknowledged} required
          onChange={e => controller.setDraft({ acknowledged: e.target.checked })} />I explicitly confirm this real-world description and its audience.</label>
        <button type="submit">{mode === 'create' ? 'Share check-in' : 'Save new revision'}</button>
      </fieldset>
    </form>
    <CommandStatus controller={controller} />
  </section>
}
const title = { idle: 'Find each other', loading: 'Reading current meeting', current: 'Current meeting', moved: 'Meeting moved or changed',
  stale: 'Meeting guidance is stale', expired: 'Meeting expired', revoked: 'Meeting revoked', unavailable: 'Meeting unavailable' } as const
export function MeetingBeaconDetail({ controller, ownerActions = false }: { readonly controller: MeetingBeaconController; readonly ownerActions?: boolean }) {
  const state = useMeeting(controller), b = state.beacon, p = state.place, id = useId()
  const [confirmRevoke, setConfirmRevoke] = useState(false)
  useEffect(() => { setConfirmRevoke(false) }, [state.epoch, b?.revision, b?.beacon_id, state.phase])
  const current = state.phase === 'current', source = sourceLink(p?.provenance.source_uri ?? null)
  const blocked = !state.canWrite || ['sending', 'uncertain', 'pending', 'checking', 'retired'].includes(state.command.phase)
  return <section className="meeting-beacon" aria-labelledby={`${id}-title`}>
    <h2 id={`${id}-title`}>{title[state.phase]}</h2>
    <p>Real-world meeting guidance. Avatar position is virtual and does not confirm arrival.</p>
    {state.phase === 'moved' && <p role="status">Earlier guidance is superseded. Review this revision and explicitly confirm it before reporting arrival.</p>}
    {b && p && (current || state.phase === 'moved') && <>
      <h3>{p.label}</h3><p>{b.meeting.instructions}</p>
      <dl>
        <dt>Identification</dt><dd>{b.meeting.identification}</dd>
        <dt>Audience</dt><dd>{b.meeting.audience.kind === 'PARTY_MEMBERS' ? 'Approved party members only' : `${b.meeting.audience.user_ids.length} explicitly selected people only`}</dd>
        <dt>Last confirmed by the owner</dt><dd><time dateTime={b.updated_at}>{b.updated_at}</time></dd>
        <dt>Expires</dt><dd><time dateTime={b.meeting.expires_at}>{b.meeting.expires_at}</time></dd>
        <dt>Revision</dt><dd>Beacon {b.revision} · Place {b.meeting.place_revision} · Map {b.meeting.map_revision}</dd>
      </dl>
      {source && <a href={source} target="_blank" rel="noopener noreferrer">Reviewed place source (new tab)</a>}
      <p>Source review: {p.provenance.reviewed_at}. Check current venue guidance; this description does not change venue entry rules.</p>
      {state.photo ? <figure><img src={state.photo.src} alt={state.photo.alt} referrerPolicy="no-referrer" /><figcaption>Authorized photo for this revision only.</figcaption></figure> : <p>No authorized photo is available for this revision.</p>}
      {state.phase === 'moved' && <button type="button" onClick={() => controller.confirmRevision()}>I have reviewed this revision</button>}
      {ownerActions && current && <div className="meeting-actions">
        <button type="button" disabled={blocked} onClick={() => controller.editCurrent()}>Use current description in update form</button>
        {!confirmRevoke ? <button type="button" disabled={blocked} onClick={() => setConfirmRevoke(true)}>Revoke meeting…</button>
          : <><p>Revoke this description for everyone in its audience?</p><button type="button" disabled={blocked} onClick={() => { setConfirmRevoke(false); void controller.revoke() }}>Confirm revocation</button><button type="button" onClick={() => setConfirmRevoke(false)}>Keep meeting</button></>}
      </div>}
    </>}
    {!b && !['idle', 'loading'].includes(state.phase) && <p role="status">Previous private instructions and photos have been cleared. Do not follow an earlier description.</p>}
    {['stale', 'unavailable', 'moved'].includes(state.phase) && <button type="button" disabled={!state.canRead} onClick={() => { void controller.refresh() }}>Read latest meeting</button>}
    <CommandStatus controller={controller} />
  </section>
}
