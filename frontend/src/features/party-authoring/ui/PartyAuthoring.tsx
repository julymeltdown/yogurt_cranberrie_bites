import { useEffect, useSyncExternalStore } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { partyDraftSchema, type PartyDraft } from '../model/contract'
import type { PartyAuthoringController } from '../model/controller'
import {kstInput,utcFromKst,kstTime} from '../model/time'
import './authoring.css'
export function PartyAuthoring({controller,editing=false}:{controller:PartyAuthoringController;editing?:boolean}) {
  const s=useSyncExternalStore(controller.subscribe,controller.read,controller.read)
  useEffect(()=>{if(editing) void controller.loadLatest()},[controller,editing])
  if(!s.access || s.phase==='denied') return <section className="party-authoring"><h1>Party authoring unavailable</h1><p>Your current access does not allow this form.</p></section>
  return <PartyForm key={s.epoch} controller={controller} editing={editing}/>
}
function PartyForm({controller,editing}:{controller:PartyAuthoringController;editing:boolean}) {
  const s=useSyncExternalStore(controller.subscribe,controller.read,controller.read)
  const {register,handleSubmit,reset,getValues,control,formState:{errors}}=useForm<PartyDraft>({resolver:zodResolver(partyDraftSchema),defaultValues:s.draft})
  useEffect(()=>{reset(controller.read().draft)},[controller,reset,s.epoch,s.party?.revision])
  const locked=['loading','sending','uncertain','pending','checking','reconciling','succeeded'].includes(s.phase)
  const labels:Record<keyof PartyDraft,string>={title:'Party title',description:'Introduction',ticket_section:'Ticket section',capacity:'Capacity',meeting_starts_at:'Start time (KST)',meeting_ends_at:'End time (KST)',requirements:'Requirements',public_place_id:'Public meeting place',state:'Party state'}
  const timeField=(name:'meeting_starts_at'|'meeting_ends_at')=><label>{labels[name]}<Controller name={name} control={control} render={({field})=><input aria-label={labels[name]} type="datetime-local" step="1" value={kstInput(field.value)} onBlur={field.onBlur} ref={field.ref} onChange={e=>{field.onChange(utcFromKst(e.target.value));controller.edit(getValues())}}/>}/>{errors[name]&&<span role="alert">{errors[name].message}</span>}</label>
  const field=(name:keyof PartyDraft,type='text')=><label key={name} htmlFor={`party-${name}`}>{labels[name]}<input id={`party-${name}`} type={type} aria-label={labels[name]} {...register(name,type==='number'?{valueAsNumber:true}:{})} aria-invalid={!!errors[name]} aria-describedby={errors[name]?`party-error-${name}`:undefined}/>{errors[name]&&<span id={`party-error-${name}`} role="alert">{errors[name].message}</span>}</label>
  return <section className="party-authoring" aria-labelledby="party-authoring-title"><header><p>MEET / YOUR PARTY</p><h1 id="party-authoring-title">{editing?'Edit your party':'Bring your people together'}</h1><p>Set a plan, leave room for others, and check the current venue guidance.</p></header>
    <p>Times use Korea Standard Time (KST). A meeting place does not confirm physical arrival.</p>
    <form noValidate onChange={()=>controller.edit(getValues())} onSubmit={handleSubmit(draft=>controller.submit(draft))}><fieldset disabled={locked}><legend>Party details</legend>
      {field('title')}{field('ticket_section')}{field('capacity','number')}{field('description')}{timeField('meeting_starts_at')}{timeField('meeting_ends_at')}
      <label htmlFor="party-place">Public meeting place<select id="party-place" {...register('public_place_id')} aria-invalid={!!errors.public_place_id}><option value="">Choose a qualified place</option>{controller.places().map(p=><option key={p.id} value={p.id}>{p.label}</option>)}</select>{errors.public_place_id&&<span role="alert">Choose an available place.</span>}</label>
      {!controller.places().length&&<p>No qualified places are available. Publishing is unavailable.</p>}{field('requirements')}
      {editing&&<label htmlFor="party-state">Party state<select id="party-state" {...register('state')}><option value="OPEN">Open</option><option value="CLOSED">Closed to applications</option><option value="COMPLETED">Completed</option></select></label>}
    </fieldset><div className="party-authoring-actions"><button type="submit" disabled={locked||s.phase==='conflict'||!controller.places().length||s.retryAt>Date.now()}>{editing?'Save changes':'Create party'}</button><button type="button" disabled={locked} onClick={()=>{controller.edit(getValues());controller.saveDraft()}}>Save session draft</button><button type="button" disabled={locked||!s.saved} onClick={()=>controller.restoreDraft()}>Restore draft</button></div></form>
    <div role="status" aria-live="polite"><p>{s.message}</p></div>
    {['uncertain','pending','reconciling'].includes(s.phase)&&<button type="button" disabled={s.retryAt>Date.now()} onClick={()=>void controller.checkStatus()}>Check original command</button>}
    {editing&&['conflict','failed'].includes(s.phase)&&<button type="button" onClick={()=>void controller.loadLatest()}>Read latest party</button>}
    {s.latest&&<section aria-label="Current server version"><h2>Current server version</h2><p>{s.latest.title} · revision {s.latest.revision} · {s.latest.member_count} of {s.latest.capacity} members</p><p>Meeting revision {s.latest.meeting_revision}: {kstTime(s.latest.meeting_starts_at)} to {kstTime(s.latest.meeting_ends_at)}</p><button type="button" onClick={()=>controller.keepDraftWithLatest()}>Keep my draft with this revision</button></section>}
  </section>
}
