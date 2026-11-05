import './parade-ink-private/private.css'
import { useEffect,useState,useSyncExternalStore } from 'react'
import type { SessionController } from '../auth/model/session'
import type { AccountController } from './controller'
import type { DeletionProgress } from './contract'
import { SafetyDialog } from '../safety/Dialog'
import '../safety/safety.css'
const deletionLabels: Record<DeletionProgress['steps'][number]['target'], string> = {
 PROFILE:'Profile', FRIEND:'Friends', POST:'Posts', NOTIFICATION:'Notifications',
 CONCERT_TICKETS:'Concert tickets', CONCERT_SESSIONS:'Concert sessions',
 CONCERT_SHARING:'Concert sharing', CONCERT_PRIVATE_DATA:'Concert private data',
}
export function AccountPanel({controller,auth,onBack}:{controller:AccountController;auth:SessionController;onBack:()=>void}){
 const s=useSyncExternalStore(controller.subscribe,controller.read),session=useSyncExternalStore(auth.subscribe,auth.read),[confirm,setConfirm]=useState<{kind:'logout'|'delete';owner:AccountController;auth:SessionController;scopeKey:string}|null>(null)
 const scopeKey=JSON.stringify(auth.privacy.capture())
 useEffect(()=>setConfirm(null),[controller,auth,scopeKey])
 const confirms=(kind:'logout'|'delete')=>confirm?.kind===kind&&confirm.owner===controller&&confirm.auth===auth&&confirm.scopeKey===JSON.stringify(auth.privacy.capture())
 const canDelete=controller.canRequestDeletion(),signedIn=auth.session.read().userId!==null
 return <section className="mcr-safety-panel pi-private-account" aria-labelledby="mcr-account-title"><button onClick={onBack}>Back</button><h1 id="mcr-account-title">Account controls</h1>
  <p>Signing out clears private views and drafts on this device immediately.</p>
  <button disabled={!signedIn||session.pending==='logout'||s.phase==='closed'} onClick={()=>setConfirm({kind:'logout',owner:controller,auth,scopeKey})}>Sign out</button>
  {session.logout==='confirmed'&&<p role="status">Signed out. Server logout confirmed.</p>}
  {session.logout==='unconfirmed'&&<p role="status">Signed out on this device. Server logout could not be confirmed.</p>}
  {session.pending==='logout'&&<p role="status">Private views cleared. Waiting for server logout confirmation…</p>}
  <h2>Delete account</h2><p>Deletion may remain pending or need recovery. A request acknowledgment does not mean deletion is complete.</p>
  {!controller.deletionAvailable&&<p>Account deletion is unavailable until the canonical Auth deletion service is connected.</p>}
  <button disabled={!canDelete||!signedIn} onClick={()=>setConfirm({kind:'delete',owner:controller,auth,scopeKey})}>Request account deletion</button>
  <p role="status">{s.message}</p>
  {s.progress&&<ul aria-label="Deletion progress">{s.progress.steps.map(step=><li key={step.target}>{deletionLabels[step.target]}: {step.state.toLowerCase().replaceAll('_',' ')}</li>)}</ul>}
  {controller.deletionAvailable&&signedIn&&['ready','unknown','pending','partial-failure'].includes(s.phase)&&<button onClick={()=>void controller.checkDeletion()}>Check deletion progress</button>}
  {confirms('logout')&&signedIn&&s.phase!=='closed'&&<SafetyDialog title="Sign out?" onDismiss={()=>setConfirm(null)}><p>Unsaved private drafts will be cleared.</p><button data-initial-focus onClick={()=>setConfirm(null)}>Stay signed in</button><button onClick={()=>{if(!confirms('logout'))return;setConfirm(null);void controller.logout()}}>Confirm sign out</button></SafetyDialog>}
  {confirms('delete')&&canDelete&&signedIn&&<SafetyDialog title="Request account deletion?" onDismiss={()=>{controller.editConfirmation('');setConfirm(null)}}><p>This request cannot be undone here. Type DELETE_MY_ACCOUNT to confirm.</p><label>Deletion confirmation<input value={s.confirmation} onChange={e=>controller.editConfirmation(e.target.value)} autoComplete="off"/></label><button data-initial-focus onClick={()=>{controller.editConfirmation('');setConfirm(null)}}>Keep account</button><button disabled={s.confirmation!=='DELETE_MY_ACCOUNT'} onClick={()=>{if(!confirms('delete'))return;setConfirm(null);void controller.requestDeletion()}}>Confirm deletion request</button></SafetyDialog>}
 </section>
}
