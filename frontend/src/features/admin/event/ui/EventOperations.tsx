import { useEffect, useRef, useState } from 'react'
import { dataFromNotice, type OperationsController } from '../model/controller'
import type { CreateNotice, Notice, Settings } from '../model/contract'
import { Field, OperationFeedback, OperationsFrame, OperationsGate, useOperations, useReviewFocus } from './common'

const limits = [
  ['beacon_ttl_seconds', 'Beacon lifetime (seconds)', 1, 900], ['pickup_grace_seconds', 'Pickup grace (seconds)', 0, 86400],
  ['social_grant_ttl_seconds', 'Social grant lifetime (seconds)', 1, 86400], ['mural_cooldown_ms', 'Wall cooldown (milliseconds)', 0, 3600000],
] as const
export function EventOperations({ controller, summaryOnly = false }: { controller: OperationsController; summaryOnly?: boolean }) {
  const state = useOperations(controller), confirm = useRef<HTMLDivElement>(null)
  useEffect(() => { void controller.loadEvent() }, [controller])
  useReviewFocus(state, confirm)
  return <OperationsGate state={state}><OperationsFrame title={summaryOnly ? 'Control room' : 'Event settings'}>
    <button type="button" disabled={state.loading || state.command?.stage === 'review' || state.command?.stage === 'sending'} onClick={() => { void controller.loadEvent() }}>Refresh event information</button>
    {state.overview && <section><h2>Operational summary</h2><dl><dt>Health</dt><dd>{state.overview.health}</dd><dt>Open reports</dt><dd>{state.overview.open_reports}</dd>
      <dt>Pending jobs</dt><dd>{state.overview.pending_jobs}</dd><dt>Overview revision</dt><dd>{state.overview.revision}</dd></dl></section>}
    {state.settings && <SettingsForm key={state.settings.event.event_id} settings={state.settings} controller={controller} locked={state.command !== null || state.loading} />}
    <div ref={confirm}><OperationFeedback state={state} controller={controller} /></div>
    {!summaryOnly && state.settings && <section><h2>Notices</h2><p>Publish only reviewed public information. Do not include private meeting details or report evidence.</p>
      <NoticeEditor controller={controller} locked={state.command !== null || state.loading} />
      {state.notices.length === 0 && <p>No notices in this authorized page.</p>}
      {state.notices.map(notice => <NoticeRecord key={`${notice.notice_id}:${notice.revision}`} notice={notice} controller={controller} locked={state.command !== null || state.loading} />)}
      {state.noticeCursor && <button type="button" disabled={state.loading || state.command !== null} onClick={() => { void controller.loadNotices(true) }}>Next notices page</button>}
    </section>}
  </OperationsFrame></OperationsGate>
}
function SettingsForm({ settings, controller, locked }: { settings: Settings; controller: OperationsController; locked: boolean }) {
  const [draft, setDraft] = useState(settings), [reason, setReason] = useState('')
  const next = settings.event.phase === 'DRAFT' ? 'OPEN' : settings.event.phase === 'OPEN' ? 'LIVE' : null
  return <section><h2>{settings.event.title}</h2><dl><dt>Event</dt><dd>{settings.event.event_id}</dd><dt>Phase</dt><dd>{settings.event.phase}</dd><dt>Settings revision</dt><dd>{settings.revision}</dd></dl>
    <p>Inputs remain a draft after refresh. Compare them with the current revision before reviewing another change.</p>
    <p>Read-only transition, freeze, archive, expiry and cleanup use the separate event closure job. No closure action runs here.</p>
    <form onSubmit={event => { event.preventDefault(); controller.prepareSettings({ expected_revision: settings.revision, features: draft.features,
      beacon_ttl_seconds: draft.beacon_ttl_seconds, pickup_grace_seconds: draft.pickup_grace_seconds,
      social_grant_ttl_seconds: draft.social_grant_ttl_seconds, mural_cooldown_ms: draft.mural_cooldown_ms, reason }) }}>
      <fieldset disabled={locked}><legend>Capability switches and limits</legend><p>Switching off a feature must also be enforced by its owning server. Hidden controls alone do not disable a feature.</p>
        {Object.entries(draft.features).map(([key, value]) => <label className="ops-check" key={key}><input type="checkbox" checked={value} onChange={e => setDraft({ ...draft, features: { ...draft.features, [key]: e.target.checked } })} />{key}</label>)}
        <div className="ops-fields">{limits.map(([key, label, min, max]) => <Field key={key} label={label}>{id => <input id={id} type="number" min={min} max={max} step="1" required value={draft[key]} onChange={e => setDraft({ ...draft, [key]: e.target.valueAsNumber })} />}</Field>)}</div>
        <Field label="Reason for settings or phase change">{id => <textarea id={id} required maxLength={500} value={reason} onChange={e => setReason(e.target.value)} />}</Field>
        <div className="ops-actions"><button type="submit">Review settings change</button>
          {next && <button type="button" onClick={() => controller.preparePhase({ expected_revision: settings.revision, phase: next, reason })}>Review phase change to {next}</button>}</div>
      </fieldset>
    </form>
  </section>
}
const blank: CreateNotice = { title: '', text: '', source_uri: '', confirmed_at: '', state: 'DRAFT' }
function NoticeEditor({ controller, locked, notice, onClose }: { controller: OperationsController; locked: boolean; notice?: Notice; onClose?: () => void }) {
  const [draft, setDraft] = useState<CreateNotice>(notice ? dataFromNotice(notice) : blank)
  const update = (key: keyof CreateNotice, value: string) => setDraft({ ...draft, [key]: value })
  return <form onSubmit={e => { e.preventDefault(); controller.prepareNotice({ ...draft, ...(notice ? { expected_revision: notice.revision } : {}) }, notice?.notice_id ?? null) }}>
    <fieldset disabled={locked}><legend>{notice ? `Edit notice revision ${notice.revision}` : 'New notice'}</legend>
      <Field label="Notice title">{id => <input id={id} required maxLength={120} value={draft.title} onChange={e => update('title', e.target.value)} />}</Field>
      <Field label="Notice text">{id => <textarea id={id} required maxLength={4000} value={draft.text} onChange={e => update('text', e.target.value)} />}</Field>
      <Field label="Reviewed HTTPS source">{id => <input id={id} required type="url" maxLength={2048} value={draft.source_uri} onChange={e => update('source_uri', e.target.value)} />}</Field>
      <Field label="Source confirmed at (UTC)">{id => <input id={id} required placeholder="YYYY-MM-DDTHH:mm:ssZ" value={draft.confirmed_at} onChange={e => update('confirmed_at', e.target.value)} />}</Field>
      <Field label="Notice publication state">{id => <select id={id} value={draft.state} onChange={e => update('state', e.target.value)}><option>DRAFT</option><option>PUBLISHED</option><option>WITHDRAWN</option></select>}</Field>
      <div className="ops-actions"><button type="submit">Review notice change</button>{onClose && <button type="button" onClick={onClose}>Close notice editor</button>}</div>
    </fieldset>
  </form>
}
function NoticeRecord({ notice, controller, locked }: { notice: Notice; controller: OperationsController; locked: boolean }) {
  const [editing, setEditing] = useState(false)
  return <article className="ops-record"><h3>{notice.title}</h3><p>{notice.text}</p><dl><dt>Notice</dt><dd>{notice.notice_id}</dd><dt>State</dt><dd>{notice.state}</dd>
    <dt>Revision</dt><dd>{notice.revision}</dd><dt>Source</dt><dd>{notice.source_uri}</dd><dt>Confirmed at</dt><dd><time dateTime={notice.confirmed_at}>{notice.confirmed_at}</time></dd></dl>
    <div className="ops-actions"><button type="button" disabled={locked} onClick={() => setEditing(!editing)}>{editing ? 'Hide notice editor' : 'Edit notice'}</button>
      {notice.state !== 'WITHDRAWN' && <button type="button" disabled={locked} onClick={() => controller.prepareNotice({ ...dataFromNotice(notice), state: 'WITHDRAWN', expected_revision: notice.revision }, notice.notice_id)}>Review notice withdrawal</button>}</div>
    {editing && <NoticeEditor notice={notice} controller={controller} locked={locked} onClose={() => setEditing(false)} />}
  </article>
}
