import { useEffect, useSyncExternalStore, type FormEvent, type ReactNode } from 'react'
import { ShellLink as Link } from '../../../app/navigation'
import { noticeText, type Filters, type PartyController, type Resource } from '../model/controller'
import type { Application, Party } from '../model/contract'
import { ConcertAction, ConcertRow, ConcertState } from '../../../shared/ui/concert-window'
import './party-discovery.css'

function useParty(controller: PartyController) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  useEffect(() => { controller.start() }, [controller]); return state
}
const time = (value: string) => new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit', timeZoneName: 'short' }).format(new Date(value))
const localInput = (value: string) => { if (!value) return ''; const date = new Date(value); return new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16) }
const partyPath = (id: string) => `/meet/parties/${id}`
function ReadState({ resource, retry, label }: { resource: Resource<unknown>; retry: () => void; label: string }) {
  return <>
    {resource.phase === 'loading' && <p role="status">Reading {label}…</p>}
    {resource.error && <div role="alert"><p>{noticeText[resource.error]}</p><button type="button" onClick={retry}>Read {label} again</button></div>}
  </>
}
/** Supplied by root from current authorized projections; never resolves raw avatar metadata here. */
export interface PartyDiscoveryPresentation {
  /** Caller binds the current owner avatar to a reviewed selected-pack renderer; null means unavailable. */
  readonly renderOwnerPortrait?: (owner: Party['owner']) => ReactNode
  /** Current reviewed place projection only; a missing lease returns null. */
  readonly placeLabel?: (placeId: string) => string | null
  /** Explicit current creation capability, never inferred from read access. */
  readonly canCreate?: boolean
}
export interface PartyDiscoveryProps { readonly controller: PartyController; readonly presentation?: PartyDiscoveryPresentation }
function MeetGlyph({ kind }: { kind: 'place' | 'members' | 'plus' }) {
  return <svg viewBox="0 0 16 16" aria-hidden="true" focusable="false" fill="currentColor">
    {kind === 'place' ? <path d="M8 1a5 5 0 0 0-5 5c0 3.5 5 9 5 9s5-5.5 5-9a5 5 0 0 0-5-5m0 3a2 2 0 1 1 0 4 2 2 0 0 1 0-4" />
      : kind === 'members' ? <><circle cx="6" cy="4" r="2.5" /><path d="M1 13v-2a5 5 0 0 1 10 0v2zm10-7a2 2 0 1 0-1-4 4 4 0 0 1 1 4m1 2a4 4 0 0 1 3 4v1h-3z" /></>
        : <path d="M7 1h2v6h6v2H9v6H7V9H1V7h6z" />}
  </svg>
}
function DiscoveryRow({ party, presentation }: { party: Party; presentation?: PartyDiscoveryPresentation | undefined }) {
  const portrait = presentation?.renderOwnerPortrait?.(party.owner)
  const place = presentation?.placeLabel?.(party.public_place_id)?.trim()
  const open = party.state === 'OPEN' && party.member_count < party.capacity
  return <li className="meet-party">
    <div className="meet-party__portrait" aria-label={`Host: ${party.owner.display_name}`}>
      {portrait ?? <span className="meet-party__portrait-unavailable" role="img" aria-label="Host portrait unavailable"><span aria-hidden="true">?</span></span>}
    </div>
    <div className="meet-party__copy">
      <h3><Link id={`party-open-${party.party_id}`} to={partyPath(party.party_id)}>{party.title}</Link></h3>
      <p className="meet-party__where"><MeetGlyph kind="place" /><span>{place || party.ticket_section || 'Place unavailable'} · <time dateTime={party.meeting_starts_at}>{new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit', timeZone: 'Asia/Seoul' }).format(new Date(party.meeting_starts_at))}</time></span></p>
      <p className="meet-party__members-unavailable">Member portraits unavailable</p>
      <span className="party-sr">{party.member_count >= party.capacity ? 'Full' : `${party.capacity - party.member_count} spaces`}. {party.state.toLowerCase()}. Hosted by {party.owner.display_name}. Meeting {time(party.meeting_starts_at)}.</span>
    </div>
    <div className="meet-party__actions"><span className="meet-party__capacity"><MeetGlyph kind="members" /><span>{party.member_count}/{party.capacity}<span className="party-sr"> members</span></span></span>
      <Link className={`meet-party__join${open ? '' : ' meet-party__join--read'}`} id={`party-details-${party.party_id}`} to={partyPath(party.party_id)} aria-label={`View details for ${party.title}`}>{open ? 'Join' : party.member_count >= party.capacity && party.state === 'OPEN' ? 'Full' : party.state.toLowerCase()}</Link>
    </div>
  </li>
}
function PartyRow({ party }: { party: Party }) {
  return <ConcertRow title={<Link id={`party-open-${party.party_id}`} to={partyPath(party.party_id)}>{party.title}</Link>}
    metadata={<>{party.ticket_section ?? 'Any ticket section'} · {party.state.toLowerCase()}<br />{party.owner.display_name} · <strong>{party.member_count}/{party.capacity} members</strong></>}
    action={<Link className="party-details" id={`party-details-${party.party_id}`} to={partyPath(party.party_id)} aria-label={`View details for ${party.title}`}>Details</Link>}>
    <time dateTime={party.meeting_starts_at}>{time(party.meeting_starts_at)}</time><br />
    <strong>{party.member_count >= party.capacity ? 'Full' : `${party.capacity - party.member_count} spaces`}</strong>
  </ConcertRow>
}
const commandCopy = {
  idle: '', sending: 'Sending once. Please wait.', uncertain: 'The response was lost. The outcome is unknown.', pending: 'The server is still processing this command.',
  checking: 'Checking the original command status…', reconciling: 'The server confirmed execution. Read MY PLAN to verify the current application state.',
  succeeded: 'The command was confirmed.', failed: 'The server rejected this command.', unavailable: 'Command status is unavailable. This does not prove whether it ran.',
  retired: 'The receipt was retired. Its outcome cannot be inferred here.', expired: 'According to this device’s clock, the receipt retention time has passed. Its outcome cannot be inferred here.',
} as const
function CommandStatus({ controller }: { controller: PartyController }) {
  const s = useParty(controller), c = s.command
  if (c.phase === 'idle') return null
  const canCheck = ['uncertain', 'pending', 'reconciling', 'unavailable', 'retired', 'expired'].includes(c.phase)
  return <section className="party-command" aria-label="Application command status" aria-live="polite" aria-busy={c.phase === 'sending' || c.phase === 'checking'}>
    <h3>{c.kind === 'cancel' ? 'Cancellation' : 'Application'} status</h3><p>{commandCopy[c.phase]}</p>
    {c.currentState && <p>Application state: <strong>{c.currentState.toLowerCase()}</strong></p>}
    {c.error && <p>{noticeText[c.error]}</p>}
    {canCheck && <><p>The original command is retained for this session. Do not submit it again.</p><button type="button" onClick={() => void controller.checkStatus()}>Check original command status</button></>}
    {c.phase === 'reconciling' && <button type="button" onClick={() => void controller.loadActivity()}>Read current MY PLAN</button>}
    {c.phase === 'failed' && <><p>Read the latest party and MY PLAN before starting a different intent.</p><button type="button" onClick={() => { void controller.loadDetail(); void controller.loadActivity() }}>Read latest information</button>
      <button type="button" disabled={!controller.canReviewNewIntent()} onClick={controller.clearFailure}>Review a new intent</button></>}
  </section>
}
function ApplicationRow({ application: a, controller }: { application: Application; controller: PartyController }) {
  const reason = controller.cancelReason(a.application_id)
  return <li className="party-row"><div><h3><Link id={`party-application-${a.application_id}`} to={partyPath(a.party_id)}>View party application</Link></h3><p>{a.state.toLowerCase()} · <time dateTime={a.created_at}>{time(a.created_at)}</time></p>
    {a.note && <p className="party-prose">{a.note}</p>}
    {a.state === 'PENDING' && <><button type="button" disabled={!!reason} aria-describedby={`cancel-${a.application_id}`} onClick={() => void controller.cancel(a.application_id)}>Cancel application</button>
      {reason && <p id={`cancel-${a.application_id}`} className="party-hint">{reason}</p>}</>}
    {a.state === 'APPROVED' && <p className="party-hint">Approved membership changes are managed separately.</p>}</div></li>
}
function MyPlan({ controller }: { controller: PartyController }) {
  const s = useParty(controller), a = s.activity.data
  return <section aria-label="MY PLAN"><div className="party-heading"><h2>MY PLAN</h2><button type="button" onClick={() => void controller.loadActivity()} disabled={s.activity.phase === 'loading'}>Refresh MY PLAN</button></div>
    <p>Owned parties, memberships and applications are paged separately.</p><ReadState resource={s.activity} label="MY PLAN" retry={() => void controller.loadActivity()} />
    {a && <><section><h3>Your parties</h3>{!a.owned.items.length && <p>No owned parties on this page.</p>}<ul className="party-list">{a.owned.items.map(p => <PartyRow key={p.party_id} party={p} />)}</ul>
      {a.owned.next_cursor && <button type="button" disabled={s.activity.phase === 'loading'} onClick={() => void controller.loadActivity('owned')}>Load more owned parties</button>}</section>
      <section><h3>Your memberships</h3>{!a.memberships.items.length && <p>No memberships on this page.</p>}<ul className="party-list">{a.memberships.items.map(m => <li className="party-row" key={m.party_id}><div>
        <Link id={`party-membership-${m.party_id}`} to={partyPath(m.party_id)}>View party membership</Link><p>{m.role.toLowerCase()} · {m.state.toLowerCase()}</p></div></li>)}</ul>
      {a.memberships.next_cursor && <button type="button" disabled={s.activity.phase === 'loading'} onClick={() => void controller.loadActivity('memberships')}>Load more memberships</button>}</section>
      <section><h3>Your applications</h3>{!a.applications.items.length && <p>No applications on this page.</p>}<ul className="party-list">{a.applications.items.map(a => <ApplicationRow key={a.application_id} application={a} controller={controller} />)}</ul>
      {a.applications.next_cursor && <button type="button" disabled={s.activity.phase === 'loading'} onClick={() => void controller.loadActivity('applications')}>Load more applications</button>}</section>
      {(a.owned.next_cursor || a.memberships.next_cursor || a.applications.next_cursor) && <p className="party-hint">More MY PLAN results remain. Load each remaining collection before applying to a party.</p>}</>}
  </section>
}
function FilterForm({ controller }: { controller: PartyController }) {
  const s = useParty(controller), f = s.filters
  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); const data = new FormData(event.currentTarget)
    const utc = (key: string) => { const value = String(data.get(key) ?? ''); return value && Number.isFinite(Date.parse(value)) ? new Date(value).toISOString() : value }
    void controller.setFilters({ ticket_section: String(data.get('ticket_section') ?? '').trim(), state: String(data.get('state') ?? '') as Filters['state'], has_space: data.get('has_space') === 'on', starts_after: utc('starts_after'), starts_before: utc('starts_before') })
  }
  return <details className="party-filters"><summary>Filter parties</summary><form onSubmit={submit} key={s.epoch}>
    <label>Ticket section<input name="ticket_section" maxLength={120} defaultValue={f.ticket_section} placeholder="Any section" /></label>
    <label>Party state<select name="state" defaultValue={f.state}><option value="">All states</option>{['OPEN', 'CLOSED', 'CANCELLED', 'COMPLETED'].map(v => <option value={v} key={v}>{v.toLowerCase()}</option>)}</select></label>
    <label>Starts on or after<input name="starts_after" type="datetime-local" defaultValue={localInput(f.starts_after)} /></label><label>Starts before<input name="starts_before" type="datetime-local" defaultValue={localInput(f.starts_before)} /></label>
    <p className="party-hint">Dates use your device’s local time. The end of the filter range is exclusive.</p>
    <label className="party-checkbox"><input name="has_space" type="checkbox" defaultChecked={f.has_space} />Only parties with space</label>
    <button type="submit" disabled={s.list.phase === 'loading'}>Show parties</button>
    {s.filterError && <p role="alert">Check the section and date range. The start must be earlier than the end.</p>}
  </form></details>
}
export function PartyDiscovery({ controller, presentation }: PartyDiscoveryProps) {
  const s = useParty(controller)
  return <div className="party-view party-browse">
    <nav className="party-tabs" aria-label="Meet views"><button type="button" aria-pressed={s.view === 'parties'} onClick={() => controller.setView('parties')}>Parties</button><Link id="party-people" to="/meet/people">People</Link><button type="button" aria-pressed={s.view === 'plan'} onClick={() => controller.setView('plan')}>MY PLAN</button></nav>
    {!s.access && <ConcertState kind="unavailable" title="Event access required">Party information is unavailable until your current event access is verified.</ConcertState>}
    <CommandStatus controller={controller} />
    {s.view === 'plan' ? <MyPlan controller={controller} /> : <section aria-label="Party discovery" className="meet-discovery">
      <h2 className="party-sr">Find your company</h2>
      <ReadState resource={s.list} label="parties" retry={() => void controller.loadList()} />
      {s.access && s.list.data && <>{!s.list.data.items.length && <ConcertState title="No matching parties">No parties match this page. Try another filter{ s.list.data.next_cursor ? ' or load the next page' : '' }.</ConcertState>}
        <ul className="meet-parties">{s.list.data.items.map(p => <DiscoveryRow key={p.party_id} party={p} presentation={presentation} />)}</ul>
        {s.list.data.next_cursor && <ConcertAction tone="paper" pending={s.list.phase === 'loading'} onClick={() => void controller.loadList(true)}>Load more parties</ConcertAction>}</>}
      <div className="meet-tools"><FilterForm controller={controller} /><button type="button" disabled={s.list.phase === 'loading'} onClick={() => void controller.loadList()} aria-label="Refresh parties">Refresh</button></div>
      <p className="party-sr">JOIN opens party details. The host approves membership; a virtual position does not confirm arrival. Times are Korea Standard Time.</p>
    </section>}
    <footer className="meet-create">
      {s.access && presentation?.canCreate ? <Link id="party-create" to="/meet/parties/new" className="meet-create__action"><MeetGlyph kind="plus" />Create party</Link>
        : <><button type="button" className="meet-create__action" disabled aria-describedby="meet-create-unavailable"><MeetGlyph kind="plus" />Create party</button><span id="meet-create-unavailable" className="meet-create__unavailable">Party creation is currently unavailable.</span></>}
    </footer>
  </div>
}

export function PartyDetail({ controller, partyId, renderOwnerPortrait }: { controller: PartyController; partyId: string; renderOwnerPortrait?: PartyDiscoveryPresentation['renderOwnerPortrait'] }) {
  const s = useParty(controller)
  useEffect(() => { void controller.loadDetail(partyId) }, [controller, partyId])
  const p = s.detail.data?.party_id === partyId ? s.detail.data : null, reason = controller.applyReason()
  return <div className="party-view"><nav className="party-tabs" aria-label="Party navigation"><Link id="party-all" to="/meet" onClick={() => controller.setView('parties')}>All parties</Link><Link id="party-plan" to="/meet" onClick={() => controller.setView('plan')}>MY PLAN</Link></nav>
    <CommandStatus controller={controller} />{!s.access && <p role="status">Party information is unavailable until your current event access is verified.</p>}<ReadState resource={s.detail} label="party" retry={() => void controller.loadDetail(partyId)} />
    {p && <article><p className="party-meta">{p.ticket_section ?? 'Any ticket section'} · {p.state.toLowerCase()}</p><h2>{p.title}</h2><div aria-label={`Host: ${p.owner.display_name}`}>{renderOwnerPortrait?.(p.owner) ?? <span role="status" aria-label="Host portrait unavailable">Character unavailable</span>}</div><p>Hosted by {p.owner.display_name}</p>
      <dl className="party-facts"><div><dt>Members</dt><dd>{p.member_count} of {p.capacity}{p.member_count >= p.capacity ? ' · Full' : ''}</dd></div>
        <div><dt>Meeting time</dt><dd><time dateTime={p.meeting_starts_at}>{time(p.meeting_starts_at)}</time> to <time dateTime={p.meeting_ends_at}>{time(p.meeting_ends_at)}</time></dd></div></dl>
      <p className="party-prose">{p.description || 'No description supplied.'}</p><h3>Requirements</h3><p className="party-prose">{p.requirements || 'No additional requirements supplied.'}</p>
      <Link id={`party-place-${p.public_place_id}`} to={`/world/places/${p.public_place_id}`}>Review place information</Link><p className="party-hint">A place reference and virtual position do not confirm real arrival. Review the current place guidance before making physical plans.</p>
      <section className="party-apply" aria-label="Apply to party"><h3>Apply to this party</h3><p>The host reviews applications. Applying does not consume an available place.</p>
        <label>Note to the host <span className="party-hint">(optional, up to 500 characters)</span><textarea maxLength={500} value={s.note} onChange={e => controller.setNote(e.target.value)} disabled={!!reason && !['idle', 'succeeded', 'failed'].includes(s.command.phase)} /></label>
        <button type="button" disabled={!!reason} aria-describedby="party-apply-reason" onClick={() => void controller.apply()}>Apply to party</button>
        <p id="party-apply-reason" className="party-hint">{reason ?? 'Your current plan has been checked. The server verifies eligibility when you apply.'}</p>
      </section></article>}
  </div>
}
