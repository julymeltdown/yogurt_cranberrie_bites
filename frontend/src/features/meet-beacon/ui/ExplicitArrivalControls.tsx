import { useEffect, useId, useState } from 'react'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { beaconSchema, type MeetBeacon } from '../model/contract'
import { sameMeetingScope } from '../model/controller'
import './meeting-beacon.css'

export type ArrivalState = 'ON_MY_WAY' | 'HERE' | 'MET' | 'NEED_HELP' | 'LEFT'
export interface ExplicitArrivalIntent { readonly state: ArrivalState; readonly meeting_revision: number; readonly physical_presence_acknowledged: true }
/** Owner rechecks policy and self revision, then calls canonical setConcertRally. This component never mutates or infers location. */
export interface ExplicitArrivalControlsProps {
  readonly beacon: MeetBeacon | null
  readonly phase: 'current' | 'moved' | 'stale' | 'expired' | 'revoked' | 'unavailable'
  readonly authorization: { readonly scope: PrivacyScope; readonly partyId: string; readonly beaconId: string; readonly beaconRevision: number; readonly meetingRevision: number; readonly expiresAt: string } | null
  readonly currentScope: PrivacyScope
  readonly now: () => number | null
  readonly pending: boolean
  readonly onIntent: (intent: ExplicitArrivalIntent) => void
}
export function ExplicitArrivalControls(props: ExplicitArrivalControlsProps) {
  const id = useId(), [ackKey, setAckKey] = useState<string | null>(null), [, tick] = useState(0)
  const { beacon: b, authorization: a } = props
  const key = JSON.stringify([props.currentScope, a, b?.beacon_id, b?.revision, props.phase])
  const ack = ackKey === key
  useEffect(() => { const timer = setInterval(() => tick(v => v + 1), 500); return () => clearInterval(timer) }, [])
  function available() {
    const time = props.now()
    return !!b && beaconSchema.safeParse(b).success && !!a && time !== null && Number.isFinite(time)
      && props.phase === 'current' && props.currentScope.gate === 'open' && props.currentScope.userId !== null
      && sameMeetingScope(a.scope, props.currentScope) && a.scope.eventId === b.event_id
      && a.beaconId === b.beacon_id && a.beaconRevision === b.revision && Number.isSafeInteger(a.meetingRevision) && a.meetingRevision > 0
      && b.state === 'ACTIVE' && b.meeting.audience.kind === 'PARTY_MEMBERS' && b.meeting.audience.party_id === a.partyId
      && Date.parse(b.meeting.expires_at) > time && Date.parse(a.expiresAt) > time && !props.pending
  }
  const ready = available()
  const labels: readonly [ArrivalState, string][] = [['ON_MY_WAY', 'On my way'], ['HERE', 'I am here'], ['MET', 'We have met'], ['NEED_HELP', 'I need help'], ['LEFT', 'I have left']]
  return <section className="meeting-beacon" aria-labelledby={`${id}-title`}>
    <h3 id={`${id}-title`}>Report your real-world status</h3>
    <p>Walking an avatar does not confirm physical arrival. These are your explicit self-reports.</p>
    {b && <p>Beacon revision {b.revision} · Meeting revision {a?.meetingRevision ?? 'unavailable'}</p>}
    {!ready && <p role="status">Read and confirm the current authorized meeting before reporting arrival.</p>}
    <label className="meeting-check"><input type="checkbox" checked={ack && ready} disabled={!ready}
      onChange={event => setAckKey(event.target.checked ? key : null)} />I am reporting my own real-world status for this meeting revision.</label>
    <div className="meeting-actions">{labels.map(([value, label]) => <button key={value} type="button" disabled={!ready || !ack}
      onClick={() => { if (ack && available() && a) { setAckKey(null); props.onIntent({ state: value, meeting_revision: a.meetingRevision, physical_presence_acknowledged: true }) } }}>{label}</button>)}</div>
  </section>
}
