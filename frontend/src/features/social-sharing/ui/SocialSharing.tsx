import { useEffect, useState, useSyncExternalStore } from 'react'
import type { SocialSharingController } from '../model/controller'
import type { Platform } from '../model/contract'
import { messages, type Slot } from '../model/runtime'
import './sharing.css'
const labels: Record<Platform, string> = { INSTAGRAM: 'Instagram', X: 'X' }
export function ReadStatus({ slot, retry }: { slot: Slot<unknown>; retry: () => void }) {
  if (slot.phase === 'loading') return <p role="status">Loading…</p>
  return slot.failure ? <div role="alert"><p>{messages[slot.failure]}</p><button type="button" onClick={retry}>Retry read</button></div> : null
}
function WriteStatus({ controller }: { controller: SocialSharingController }) {
  const { mutation } = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  return <div aria-live="polite">{mutation.phase !== 'idle' && <p>{mutation.phase === 'sending' ? 'Sending…' : mutation.phase === 'succeeded' ? 'The server confirmed this change.' : ['uncertain', 'pending'].includes(mutation.phase) ? 'The outcome is not confirmed. Your command identity is retained. Check status before submitting again.' : mutation.failure ? messages[mutation.failure] : 'The change was not confirmed.'}</p>}
    {['uncertain', 'pending'].includes(mutation.phase) && <button type="button" onClick={() => { void controller.checkStatus() }}>Check command status</button>}
  </div>
}
function PrivacyCopy() { return <p className="sharing-note">Sharing lets the other person see your selected account name. It does not follow anyone automatically. Revoking stops future access here; it cannot erase a name someone already saw or copied. Unblocking does not restore previous consent.</p> }
export function HandleSettings({ controller }: { controller: SocialSharingController }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), [remove, setRemove] = useState<Platform | null>(null)
  const busy = ['sending', 'pending', 'uncertain'].includes(state.mutation.phase)
  return <section className="social-sharing" aria-label="Private social handles"><header><p>Private by default</p><h1>SOCIAL LINKS</h1><p>Account names are optional and never part of your public profile.</p></header><PrivacyCopy />
    {!state.available && <p role="status">Social settings are unavailable for the current account or event.</p>}{state.readOnly && <p role="status">This event is read-only. You can remove existing handles.</p>}
    <button type="button" disabled={!state.available || busy} onClick={() => { void controller.loadHandles() }}>Reload handle settings</button><ReadStatus slot={state.handles} retry={() => { void controller.loadHandles() }} />
    {state.handles.data?.items.map(handle => <section key={handle.platform} aria-label={`${labels[handle.platform]} settings`}><h2>{labels[handle.platform]}</h2><p>{handle.configured ? 'Configured privately' : 'Not configured'} · Version {handle.version}</p>
      <form onSubmit={e => { e.preventDefault(); void controller.saveHandle(handle.platform) }}>
        <label htmlFor={`handle-${handle.platform}`}>{handle.configured ? 'Replacement' : 'New'} {labels[handle.platform]} account name</label>
        <input id={`handle-${handle.platform}`} type="text" autoComplete="off" autoCapitalize="none" spellCheck={false} value={state.handleDrafts[handle.platform]} disabled={!state.available || state.readOnly || busy} onChange={e => controller.setHandleDraft(handle.platform, e.target.value)} aria-describedby={`handle-hint-${handle.platform}`} />
        <p id={`handle-hint-${handle.platform}`}>Without @. {handle.platform === 'X' ? '1–15 letters, numbers or underscores.' : '1–30 letters, numbers, dots or underscores. No starting, ending or repeated dots.'} Replacing or removing invalidates sharing bound to the previous version.</p>
        <button type="submit" disabled={!state.available || state.readOnly || busy || !state.handleDrafts[handle.platform].trim()}>{handle.configured ? 'Replace privately' : 'Save privately'}</button>
        {handle.configured && <button type="button" disabled={!state.available || busy} onClick={() => setRemove(handle.platform)}>Remove {labels[handle.platform]}</button>}
      </form>
    </section>)}
    {remove && state.handles.data?.items.some(h => h.platform === remove && h.configured) && <fieldset><legend>Confirm removal</legend><p>Remove your {labels[remove]} handle and invalidate sharing bound to it?</p><button type="button" disabled={busy} onClick={() => { void controller.removeHandle(remove); setRemove(null) }}>Confirm removal</button><button type="button" onClick={() => setRemove(null)}>Keep handle</button></fieldset>}
    <WriteStatus controller={controller} />
  </section>
}
export function SharingOffer({ controller, retry }: { controller: SocialSharingController; retry: () => void }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), offer = state.offer.data, busy = ['sending', 'pending', 'uncertain'].includes(state.mutation.phase)
  const platforms = offer?.available_handles.filter(h => state.handles.data?.items.some(own => own.platform === h.platform && own.configured)) ?? []
  return <section className="social-sharing" aria-label="Request sharing"><h1>REQUEST SHARING</h1><PrivacyCopy />
    <ReadStatus slot={state.offer} retry={retry} />
    {!state.available && <p role="status">Sharing is unavailable for the current account or event.</p>}
    {offer && <><p>The current introduction offers platform metadata only. No account name has been revealed.</p>
      {platforms.length === 0 && <p>No mutual configured platform is available. Configure your own handle first or reload the current introduction.</p>}
      <form onSubmit={e => { e.preventDefault(); void controller.createRequest() }}><fieldset disabled={busy || state.readOnly || !state.available}><legend>Choose what you explicitly consent to share</legend>
        {platforms.map(h => <label key={h.platform}><input type="checkbox" checked={state.requesterConsent.includes(h.platform)} onChange={e => controller.setRequesterConsent(h.platform, e.target.checked)} /> I consent to share my {labels[h.platform]} account name with this person if they approve a reciprocal exchange.</label>)}
      </fieldset><button type="submit" disabled={busy || state.readOnly || !state.available || state.requesterConsent.length === 0}>Send sharing request</button></form></>}
    <WriteStatus controller={controller} />
  </section>
}
export function SharingRequests({ controller, onRequest }: { controller: SocialSharingController; onRequest: (id: string) => void }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  return <section className="social-sharing" aria-label="Sharing requests"><h1>REQUESTS</h1><PrivacyCopy />
    <label>Direction <select value={state.direction} onChange={e => controller.setDirection(e.target.value as 'INCOMING' | 'OUTGOING')}><option value="INCOMING">Incoming</option><option value="OUTGOING">Outgoing</option></select></label>
    <button type="button" disabled={!state.available} onClick={() => { void controller.loadRequests() }}>Refresh requests</button>
    {!state.available && <p role="status">Requests are unavailable for this account or event.</p>}
    <ReadStatus slot={state.requests} retry={() => { void controller.loadRequests() }} />{state.requests.data?.items.length === 0 && <p>No sharing requests.</p>}
    <ol className="sharing-list">{state.requests.data?.items.map(request => <li key={request.request_id}><p>{request.state === 'PENDING' || request.state === 'APPROVED' ? Date.parse(request.expires_at) <= state.now ? 'EXPIRED' : request.state : request.state} · {request.consents.map(c => labels[c.platform]).join(', ')}</p><p>Expires <time dateTime={request.expires_at}>{new Date(request.expires_at).toLocaleString()}</time></p><button type="button" onClick={() => onRequest(request.request_id)}>Open request</button></li>)}</ol>
    {state.requests.data?.next_cursor && <button type="button" disabled={state.requests.phase === 'loading'} onClick={() => { void controller.loadRequests(true) }}>Load older requests</button>}
  </section>
}
export function SharingRequest({ controller, retry }: { controller: SocialSharingController; retry: () => void }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), [confirmRevoke, setConfirmRevoke] = useState<string | null>(null)
  useEffect(() => () => controller.hideSecret(), [controller])
  const request = state.detail.data, busy = ['sending', 'pending', 'uncertain'].includes(state.mutation.phase), expired = !!request && Date.parse(request.expires_at) <= state.now
  const recipient = request?.recipient_id === state.userId
  return <section className="social-sharing" aria-label="Sharing request detail"><h1>SHARING REQUEST</h1><PrivacyCopy /><ReadStatus slot={state.detail} retry={retry} />
    {request && <><p role="status">{expired && ['PENDING', 'APPROVED'].includes(request.state) ? 'EXPIRED' : request.state}</p><p>Expires <time dateTime={request.expires_at}>{new Date(request.expires_at).toLocaleString()}</time></p>
      {!expired && request.state === 'PENDING' && recipient && <form onSubmit={e => { e.preventDefault(); void controller.decide(true) }}>
        <fieldset disabled={busy || state.readOnly || !state.available}><legend>Approve only the platforms you choose</legend>{request.consents.map(c => <label key={c.platform}><input type="checkbox" checked={state.approvedPlatforms.includes(c.platform)} onChange={e => controller.setDecisionPlatform(c.platform, e.target.checked)} /> Exchange {labels[c.platform]} account names</label>)}
          <label><input type="checkbox" checked={state.recipientConsent} onChange={e => controller.setRecipientConsent(e.target.checked)} /> I explicitly consent to share my selected account names with the requester.</label>
        </fieldset><button type="submit" disabled={busy || state.readOnly || !state.available || !state.recipientConsent || !state.approvedPlatforms.length}>Approve selected platforms</button><button type="button" disabled={busy || state.readOnly || !state.available} onClick={() => { void controller.decide(false) }}>Reject request</button>
      </form>}
      {!expired && request.state === 'PENDING' && !recipient && <button type="button" disabled={busy || !state.available} onClick={() => { void controller.cancelRequest() }}>Cancel request</button>}
      {!expired && request.state === 'APPROVED' && <section aria-label="Approved platforms"><h2>Approved exchange</h2><p>Approval is metadata, not a displayed account name. Reveal explicitly when needed.</p>
        {!state.revealAvailable && <p>Protected reveal is currently unavailable.</p>}{request.approved_platforms.map(p => <button key={p} type="button" disabled={busy || !state.revealAvailable} onClick={() => { void controller.revealHandle(p) }}>Reveal {labels[p]}</button>)}
      </section>}
      {!expired && ['PENDING', 'APPROVED'].includes(request.state) && <button type="button" disabled={busy || !state.available} onClick={() => setConfirmRevoke(request.request_id)}>Revoke sharing</button>}
      {confirmRevoke === request.request_id && <fieldset><legend>Confirm revocation</legend><p>Stop future access for both people on all approved platforms? Previously seen names cannot be erased.</p><button type="button" disabled={busy} onClick={() => { void controller.revokeRequest(); setConfirmRevoke(null) }}>Confirm revocation</button><button type="button" onClick={() => setConfirmRevoke(null)}>Keep current sharing</button></fieldset>}
    </>}
    {state.secret.phase === 'loading' && <p role="status">Fetching the account name…</p>}{state.secret.failure && <p role="alert">{messages[state.secret.failure]} Reveal again only with an explicit platform action.</p>}
    {state.secret.data && <section aria-label="Revealed account name"><h2>{labels[state.secret.data.platform]}</h2><p className="sharing-handle">@{state.secret.data.handle}</p><p>Visible only until <time dateTime={state.secret.data.expires_at}>{new Date(state.secret.data.expires_at).toLocaleString()}</time>. Opening an external service does not follow automatically.</p></section>}
    {state.secret.phase !== 'idle' && <button type="button" onClick={controller.hideSecret}>Hide account name</button>}
    <WriteStatus controller={controller} />
  </section>
}
