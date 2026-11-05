import './parade-ink-private/private.css'
import { useEffect, useState, useSyncExternalStore } from 'react'
import type { SafetyController } from './controller'
import { reasons } from './contract'
import { SafetyDialog } from './Dialog'
import './safety.css'
import './parade-ink/report/report.css'
const labels={HARASSMENT:'Harassment',PRIVACY:'Privacy concern',UNSAFE_MEETING:'Unsafe meeting',SPAM:'Spam',OTHER:'Other'}
export interface EvidenceChoice {readonly id:string;readonly label:string}
/** Owner supplies only current actor-owned READY REPORT media choices, never moderator detail. */
export function ReportDialog({controller,search,evidenceChoices=[],onDismiss}:{controller:SafetyController;search:string;evidenceChoices?:readonly EvidenceChoice[];onDismiss:()=>void}){
 const s=useSyncExternalStore(controller.subscribe,controller.read)
 useEffect(()=>{void controller.open(search)},[controller,search])
 const dismiss=()=>{controller.close();onDismiss()}
 return <SafetyDialog title="Report a concern" onDismiss={dismiss}>
  <div className="pi-report">
  <button type="button" data-initial-focus onClick={dismiss}>Close report</button>
  {s.target&&<p>Target: {s.target.target_type.toLowerCase()} <span className="mcr-safety-id">{s.target.target_id}</span></p>}
  {s.phase==='ready'&&<form onSubmit={e=>{e.preventDefault();void controller.report()}}>
   <label>Reason<select value={s.reason} onChange={e=>controller.edit({reason:e.target.value as typeof reasons[number]})}>{reasons.map(reason=><option key={reason} value={reason}>{labels[reason]}</option>)}</select></label>
   <label>What happened? (optional, up to 2000 characters)<textarea value={s.text} onChange={e=>controller.edit({text:e.target.value})}/></label>
   {evidenceChoices.length>0&&<fieldset><legend>Evidence (up to four items)</legend>{evidenceChoices.map(item=><label key={item.id}><input type="checkbox" checked={s.evidence.includes(item.id)} disabled={!s.evidence.includes(item.id)&&s.evidence.length>=4} onChange={e=>controller.edit({evidence:e.target.checked?[...s.evidence,item.id]:s.evidence.filter(id=>id!==item.id)})}/>{item.label}</label>)}</fieldset>}
   <p className="pi-report__privacy">Only authorized reviewers can see your identity, explanation and evidence. Submission does not guarantee a moderation outcome.</p>
   <button type="submit">Submit report</button>
  </form>}
  {s.report&&<p className="pi-report__receipt">Current report status: {s.report.state.toLowerCase().replace('_',' ')}</p>}
  <p className="pi-report__status" role="status" aria-live="polite">{s.message||'Checking report availability…'}</p>
  {['pending','unknown','failed'].includes(s.phase)&&<button type="button" onClick={()=>void controller.checkStatus()}>Check original request status</button>}
  </div>
 </SafetyDialog>
}
export function BlocksPanel({controller,onBack}:{controller:SafetyController;onBack:()=>void}){
 const s=useSyncExternalStore(controller.subscribe,controller.read),[confirm,setConfirm]=useState<{owner:SafetyController;id:string;revision:number}|null>(null)
 useEffect(()=>{setConfirm(null);void controller.loadBlocks()},[controller])
 useEffect(()=>{setConfirm(value=>value?.owner===controller&&s.blocks?.items.some(item=>item.target_user_id===value.id&&item.revision===value.revision)?value:null)},[controller,s.blocks])
 const locked=!controller.blocksAvailable||['submitting','pending','unknown','failed','closed'].includes(s.phase)
 return <section className="mcr-safety-panel pi-private-blocks" aria-labelledby="mcr-blocks-title">
  <button type="button" onClick={onBack}>Back</button><h1 id="mcr-blocks-title">Blocked users</h1>
  <p>Blocking closes affected private views. Unblocking never restores previous consent or social access.</p>
  {s.blocksPhase==='loading'&&<p role="status">Loading blocked users…</p>}
  {s.blocksPhase==='ready'&&s.blocks?.items.length===0&&<p>No blocked users on this page.</p>}
  <ul>{s.blocks?.items.map(item=><li key={item.target_user_id}><span className="mcr-safety-id">{item.target_user_id}</span> <button type="button" disabled={locked} onClick={()=>setConfirm({owner:controller,id:item.target_user_id,revision:item.revision})}>Unblock user</button></li>)}</ul>
  {s.blocks?.next_cursor&&<button disabled={locked} onClick={()=>void controller.loadBlocks(s.blocks!.next_cursor!)}>Next page</button>}
  {s.blocksPhase==='error'&&<button type="button" onClick={()=>void controller.loadBlocks()}>Reload blocked users</button>}
  <p role="status">{s.message}</p>
  {['pending','unknown','failed'].includes(s.phase)&&<button onClick={()=>void controller.checkStatus()}>Check original request status</button>}
  {confirm?.owner===controller&&s.blocks?.items.some(item=>item.target_user_id===confirm.id&&item.revision===confirm.revision)&&!locked&&<SafetyDialog title="Unblock this user?" onDismiss={()=>setConfirm(null)}><p>Previous consent remains revoked. New permission is required before sharing private information again.</p><button data-initial-focus onClick={()=>setConfirm(null)}>Keep blocked</button><button onClick={()=>{const intent=confirm;setConfirm(null);if(intent.owner===controller&&controller.read().blocks?.items.some(item=>item.target_user_id===intent.id&&item.revision===intent.revision))void controller.unblock(intent.id)}}>Confirm unblock</button></SafetyDialog>}
 </section>
}
export function BlockUserButton({controller,userId}:{controller:SafetyController;userId:string}){
 const s=useSyncExternalStore(controller.subscribe,controller.read),[confirm,setConfirm]=useState<{owner:SafetyController;id:string}|null>(null)
 useEffect(()=>setConfirm(null),[controller,userId])
 const locked=!controller.blocksAvailable||['submitting','pending','unknown','failed','closed'].includes(s.phase)
 return <><button disabled={locked} onClick={()=>setConfirm({owner:controller,id:userId})}>Block user</button>{confirm?.owner===controller&&confirm.id===userId&&!locked&&<SafetyDialog title="Block this user?" onDismiss={()=>setConfirm(null)}><p>This ends access to affected private interactions. Previous consent will not return if you unblock later.</p><button data-initial-focus onClick={()=>setConfirm(null)}>Cancel block</button><button onClick={()=>{const intent=confirm;setConfirm(null);if(intent.owner===controller&&intent.id===userId)void controller.block(intent.id)}}>Confirm block</button></SafetyDialog>}<p role="status">{s.message}</p>{['pending','unknown','failed'].includes(s.phase)&&<button onClick={()=>void controller.checkStatus()}>Check original request status</button>}</>
}
