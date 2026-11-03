import { useEffect, useState, useSyncExternalStore } from 'react'
import type { PartyManagementController, RallyState } from '../model/controller'
import './management.css'
export function PartyManagement({controller}:{controller:PartyManagementController}){
 const s=useSyncExternalStore(controller.subscribe,controller.read,controller.read)
 useEffect(()=>{void controller.load()},[controller])
 const locked=!['ready','succeeded','failed'].includes(s.phase),owner=controller.isOwner()
 if(s.phase==='denied')return <section className="party-management"><h1>Party access unavailable</h1><p>{s.message}</p></section>
 return <section className="party-management"><header><p>MEET / PARTY TOOLS</p><h1>{s.party?.title??'Your party'}</h1></header><p role="status" aria-live="polite">{s.message}</p>
  <button disabled={locked&&s.phase!=='idle'||s.retryAt>Date.now()} onClick={()=>void controller.load()}>Read current party</button>
  {s.party&&<><p>{s.party.state} · {s.party.member_count} of {s.party.capacity} members · revision {s.party.revision}</p><section aria-label="Current meeting"><h2>Current meeting</h2><p>Meeting revision {s.party.meeting_revision}. Previous meeting cards are superseded.</p><p>{s.party.meeting_starts_at} to {s.party.meeting_ends_at} (UTC)</p><p>Use current authorized meeting details. Virtual movement never confirms physical arrival.</p></section></>}
  {s.party&&!owner&&<p>Owner tools are unavailable for this account.</p>}
  {owner&&<section aria-label="Party applications"><h2>Applications</h2>{!s.applications?<p>Read the current application list.</p>:!s.applications.items.length?<p>No applications on this page.</p>:<ul>{s.applications.items.map(a=><li key={a.application_id}><h3>{a.applicant.display_name}</h3><p>{a.note}</p><p>{a.state}</p>{a.state==='PENDING'&&<div className="party-management-actions"><button disabled={locked||s.party?.state!=='OPEN'||s.party.member_count>=s.party.capacity} onClick={()=>void controller.decide(a.application_id,'APPROVED')}>Approve {a.applicant.display_name}</button><button disabled={locked} onClick={()=>void controller.decide(a.application_id,'REJECTED')}>Reject {a.applicant.display_name}</button></div>}</li>)}</ul>}{s.applications?.next_cursor&&<button disabled={locked} onClick={()=>void controller.more('applications')}>More applications</button>}</section>}
  <section aria-label="Party members"><h2>Members</h2>{s.members?<ul>{s.members.items.map(m=><li key={m.user_id}><p>{m.user_id===s.party?.owner.user_id?'Party owner':m.user_id} · {m.state}</p>{owner&&m.role!=='OWNER'&&m.state==='ACTIVE'&&<button disabled={locked} onClick={()=>controller.confirm('remove',m.user_id)}>Remove member {m.user_id}</button>}</li>)}</ul>:<p>Member list unavailable.</p>}{s.members?.next_cursor&&<button disabled={locked} onClick={()=>void controller.more('members')}>More members</button>}</section>
  {owner&&s.party&&['OPEN','CLOSED'].includes(s.party.state)&&<button disabled={locked} onClick={()=>controller.confirm('cancel')}>Cancel party</button>}
  {s.confirmation&&<section role="dialog" aria-modal="false" aria-labelledby="party-confirm-title"><h2 id="party-confirm-title">{s.confirmation.kind==='cancel'?'Cancel this party?':'Remove this member?'}</h2><p>This revokes party meeting access. The server will check current revisions.</p><button onClick={()=>void(s.confirmation!.kind==='cancel'?controller.cancel():controller.remove(s.confirmation!.id))}>Confirm {s.confirmation.kind==='cancel'?'cancellation':'removal'}</button><button onClick={()=>controller.dismiss()}>Keep unchanged</button></section>}
  {['uncertain','pending'].includes(s.phase)&&<button disabled={s.retryAt>Date.now()} onClick={()=>void controller.checkStatus()}>Check original command</button>}
 </section>
}
export function PartyRally({controller}:{controller:PartyManagementController}){
 const s=useSyncExternalStore(controller.subscribe,controller.read,controller.read),[ack,setAck]=useState(false)
 useEffect(()=>setAck(false),[s.party?.meeting_revision,s.rally?.meeting_revision])
 const authority=controller.meeting(),enabled=!!authority&&!!s.rally&&['ready','succeeded','failed'].includes(s.phase)
 return <section className="party-management"><h1>Rendezvous</h1><p>Meeting revision {s.party?.meeting_revision??'unavailable'}. Previous reports do not prove arrival at a changed meeting.</p>
  {!authority?<p>Current authorized physical meeting details are unavailable or expired.</p>:<p>{authority.description}</p>}
  <button onClick={()=>void controller.loadRally()} disabled={!authority}>Read current rally</button>
  {s.rally&&<ul>{s.rally.entries.map(e=><li key={e.user_id}>{e.user_id}: {e.state.replaceAll('_',' ')} · self-reported {e.self_reported_at}</li>)}</ul>}
  <label><input type="checkbox" checked={ack} disabled={!enabled} onChange={e=>setAck(e.target.checked)}/>I am explicitly reporting my physical meeting status.</label>
  <div className="party-management-actions">{(['ON_MY_WAY','HERE','MET','NEED_HELP','LEFT'] as RallyState[]).map(v=><button key={v} disabled={!enabled||!ack} onClick={()=>{setAck(false);void controller.report(v,true)}}>{v.replaceAll('_',' ')}</button>)}</div><p role="status">{s.message}</p>
 </section>
}
