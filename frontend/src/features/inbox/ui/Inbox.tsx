import { useSyncExternalStore } from 'react'
import type { InboxController, InboxFailure } from '../model/controller'
import type { InboxNotification } from '../model/contract'
import './inbox.css'
const messages: Record<InboxFailure, string> = {
  offline: 'Connection unavailable. Reconnect, then refresh the inbox.',
  denied: 'Inbox access is unavailable for this account.', unavailable: 'The inbox service is currently unavailable.',
  invalid: 'The response could not be verified. No new notifications were displayed.',
  conflict: 'This update changed. Refresh before taking another action.',
  'rate-limited': 'Too many requests. Wait before trying again.', failed: 'The request could not be completed.',
}
const titles: Record<InboxNotification['kind'], string> = {
  PARTY: 'Party update', GIVEAWAY: 'Giveaway update', SOCIAL: 'Social sharing update',
  MODERATION: 'Account or content update', NOTICE: 'Event notice', FEED: 'Feed update',
}
export function Inbox({ controller }: { controller: InboxController }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const blocked = ['sending', 'checking', 'uncertain', 'retired'].includes(state.command.phase)
  const loading = state.phase === 'loading'
  return <section className="mcr-inbox" aria-label="Notification inbox">
    <header><p className="inbox-eyebrow">Your event updates</p><h1>INBOX</h1><p>Private updates for your current account and event.</p></header>
    {!state.available ? <p role="status">Inbox is unavailable for the current account or event.</p> : <>
      <div className="inbox-controls"><label><input type="checkbox" checked={state.unreadOnly} onChange={e => controller.setUnreadOnly(e.target.checked)} /> Unread only</label>
        <button type="button" disabled={loading} onClick={() => void controller.load()}>Refresh inbox</button>
        <button type="button" disabled={blocked || loading || state.phase !== 'ready' || !state.unreadShown || state.stale} onClick={() => void controller.markAllRead()}>Mark all as read</button></div>
      <p aria-live="polite" className="inbox-count">{state.unreadShown} unread shown{state.nextCursor ? ' · More updates may be available' : ''}</p>
      <p className="inbox-note">This count covers loaded updates. Mark all as read applies to updates already received by the server.</p>
      {loading && <p role="status">Loading inbox…</p>}
      {state.stale && <p role="status">New or changed updates are available. Refresh the inbox.</p>}
      {state.failure && <p role="alert">{messages[state.failure]}</p>}
      {state.command.phase === 'sending' && <p role="status">Saving read state…</p>}
      {state.command.phase === 'checking' && <p role="status">Checking command status…</p>}
      {(state.command.phase === 'uncertain' || state.command.phase === 'retired') && <p role="status">The read-state outcome is unconfirmed. Refreshing does not repeat the command.</p>}
      {state.command.phase === 'succeeded' && <p role="status">Read-state command confirmed.</p>}
      {state.command.phase === 'failed' && <p role="alert">{state.command.failure ? messages[state.command.failure] : 'Read state was not confirmed.'} The original command will not be repeated. You can inspect current updates or check its status.</p>}
      {['uncertain', 'failed'].includes(state.command.phase) && <button type="button" onClick={() => void controller.recover()}>Check command status</button>}
      {state.command.phase === 'retired' && <p>Command history is unavailable. Inspect current updates; the old command will not be retried.</p>}
      {state.phase === 'ready' && state.rows.length === 0 && <p>{state.unreadOnly ? 'No unread updates in this page.' : 'Your inbox is empty.'}</p>}
      <ol className="inbox-list" aria-label="Event updates">{state.rows.map((n, index) => <li key={n.notification_id}><article aria-label={`${titles[n.kind]} ${index + 1}`}>
        <div className="inbox-row-heading"><h2>{titles[n.kind]}</h2><span>{n.read_at ? 'Read' : 'Unread'}</span></div>
        <p><time dateTime={n.created_at}>{new Date(n.created_at).toLocaleString()}</time></p>
        <div className="inbox-row-actions">{n.target ? <button type="button" disabled={state.target.phase === 'checking'} onClick={() => void controller.open(n.notification_id)}>Open update</button> : <p>Destination is no longer available.</p>}
          {!n.read_at && <button type="button" disabled={blocked || loading || state.stale} onClick={() => void controller.markRead(n.notification_id)}>Mark as read</button>}</div>
        {state.target.notificationId === n.notification_id && state.target.phase === 'checking' && <p role="status">Checking access…</p>}
        {state.target.notificationId === n.notification_id && state.target.phase === 'unavailable' && <p role="status">This destination was removed, changed, or cannot be opened.</p>}
      </article></li>)}</ol>
      {state.atLimit && <p>Showing a bounded recent inbox view. Refresh for the newest updates.</p>}
      {state.nextCursor && !state.atLimit && <button type="button" disabled={loading} onClick={() => void controller.load(true)}>Load older updates</button>}
    </>}
  </section>
}
