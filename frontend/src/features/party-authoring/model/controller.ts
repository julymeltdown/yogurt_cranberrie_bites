import { validCapabilities } from '../../../app/capabilities'
import { TransportError } from '../../../shared/api/errors'
import type { SessionController } from '../../auth/model/session'
import type { PartyAccessPort } from '../../party-discovery/model/controller'
import { decodeParty } from '../../party-discovery/model/contract'
import { contractValue, emptyDraft, partyDraftSchema, receiptDecoder, uuid, type Binding, type Party, type PartyDraft, type Receipt } from './contract'
export interface QualifiedPlace { readonly id: string; readonly label: string }
export interface AuthoringOptions {
  auth: Pick<SessionController, 'session' | 'privacy' | 'request'>; access: PartyAccessPort
  /** Supplied by the reviewed place consumer. A label is not physical-arrival authority. */
  places: () => readonly QualifiedPlace[]; partyId?: string; newKey?: () => string; now?: () => number
}
type Phase = 'idle' | 'loading' | 'sending' | 'uncertain' | 'pending' | 'checking' | 'reconciling' | 'succeeded' | 'failed' | 'conflict' | 'denied'
export interface AuthoringState { readonly epoch: number; readonly phase: Phase; readonly draft: PartyDraft; readonly saved: PartyDraft | null; readonly party: Party | null; readonly latest: Party | null; readonly message: string; readonly retryAt: number; readonly access: boolean }
export function createPartyAuthoring(options: AuthoringOptions) {
  const { auth, access, partyId } = options, now = options.now ?? Date.now
  let state: AuthoringState = Object.freeze({ epoch: 0, phase: 'idle', draft: Object.freeze(emptyDraft()), saved: null, party: null, latest: null, message: '', retryAt: 0, access: false })
  let disposed = false, flight: AbortController | null = null, binding: Binding | null = null
  const listeners = new Set<() => void>()
  const emit = (patch: Partial<AuthoringState>) => { if (disposed) return; state = Object.freeze({ ...state, ...patch }); for (const f of Array.from(listeners)) { try { f() } catch { /* Keep privacy cleanup synchronous for every observer. */ } } }
  function allowed() { const s = auth.privacy.capture(), a = access.read(); return !disposed && auth.privacy.isCurrent(s) && uuid(s.userId) && uuid(s.eventId) && !!a && a.scope.userId === s.userId && a.scope.eventId === s.eventId && a.scope.generation === s.generation && a.scope.policyGeneration === s.policyGeneration && a.scope.gate === s.gate && validCapabilities(a.capabilities) && a.capabilities.meet.available && a.capabilities.mutations.available && ['OPEN','LIVE'].includes(a.phase) }
  const busy = () => ['loading','sending','uncertain','pending','checking','reconciling','succeeded','denied'].includes(state.phase)
  function reset() { flight?.abort(); flight = null; binding = null; emit({ epoch: state.epoch + 1, phase: 'idle', draft: Object.freeze(emptyDraft()), saved: null, party: null, latest: null, message: '', retryAt: 0, access: allowed() }) }
  const cleanup = auth.privacy.registerCleanup(reset)
  const stopAccess = access.subscribe(() => { if (!allowed()) { reset(); emit({ access: false }) } else emit({ access: true }) })
  function from(p: Party): PartyDraft { return { title:p.title, description:p.description, ticket_section:p.ticket_section ?? '', capacity:p.capacity, meeting_starts_at:p.meeting_starts_at, meeting_ends_at:p.meeting_ends_at, requirements:p.requirements, public_place_id:p.public_place_id, state:p.state === 'CANCELLED' ? 'CLOSED' : p.state } }
  async function readParty(id: string, signal: AbortSignal) {
    const s = auth.privacy.capture(), path = `/api/concert/events/${s.eventId}/parties/${id}`
    return auth.privacy.run(s, async ({ signal: privateSignal }) => (await auth.request({ path, method:'GET', access:'protected', errorFamily:'domain', domainOperation:'getConcertParty', signal:privateSignal, decode:decodeParty(id,path) })).data, signal)
  }
  function own(p: Party) { if (p.owner.user_id !== auth.session.read().userId || p.state === 'CANCELLED') throw new TransportError('http', { status:403 }) }
  async function loadLatest() {
    if (!partyId || !uuid(partyId) || !allowed() || !['idle','failed','conflict'].includes(state.phase) || state.retryAt > now()) return
    const scope = auth.privacy.capture(), c = new AbortController(); flight?.abort(); flight = c
    const preserve = state.party !== null; emit({ phase:'loading', message:'' })
    try { const p = await readParty(partyId,c.signal); own(p)
      if (flight !== c || !auth.privacy.isCurrent(scope) || !allowed()) return
      emit({ phase: preserve ? 'conflict':'idle', party: preserve ? state.party:p, latest: preserve ? p:null, ...(preserve ? {} : { draft:Object.freeze(from(p)) }), message:preserve ? 'Current server version loaded. Review and keep your draft before saving again.':'' })
    } catch (e) { if (flight === c && auth.privacy.isCurrent(scope)) failure(e,false) }
  }
  function failure(e: unknown, sent: boolean) {
    const t = e instanceof TransportError ? e : null, reason = t?.server?.reason
    const uncertain = sent && (!t || t.uncertain || !t.status || t.status >= 500 || t.status === 404 || reason === 'IDEMPOTENCY_CONFLICT' || reason === 'IDEMPOTENCY_EXPIRED')
    const denied = t?.status === 401 || t?.status === 403
    emit({ phase:uncertain ? 'uncertain':denied ? 'denied':reason === 'VERSION_CONFLICT' || reason === 'PARTY_FULL' ? 'conflict':'failed',
      ...(denied ? { draft:Object.freeze(emptyDraft()), saved:null, party:null, latest:null, epoch:state.epoch+1 }:{}),
      retryAt:t?.status === 429 ? now()+Math.max(1000,t.retryAfterMs ?? 1000):0,
      message:uncertain ? 'Outcome unknown. Check the original command; do not submit another.' :denied ? 'Owner access is unavailable.' :reason === 'VERSION_CONFLICT' || reason === 'PARTY_FULL' ? 'The party changed. Your draft is preserved; read the latest version.' :reason === 'INVALID_INPUT' ? 'The server rejected these values. Review your form.' :t?.status === 429 ? 'Too many requests. Wait before trying again.' :'The request could not be completed. Your draft is preserved.' })
  }
  async function acceptReceipt(r: Receipt, c: AbortController) {
    if (Date.parse(r.expires_at) <= now() || r.state === 'RETIRED') { emit({phase:'uncertain',message:'The receipt expired or was retired. The command may have executed. Do not repeat it.'}); return }
    if (r.state === 'FAILED') { emit({phase:r.failure_reason === 'VERSION_CONFLICT' ? 'conflict':'failed',message:'The command failed. Your draft is preserved. Read current information before a new intent.'}); return }
    if (r.state === 'PENDING') { emit({phase:'pending',message:'The server is processing this command. Check its status.'}); return }
    emit({phase:'reconciling',message:'Receipt found. Reading the current authorized party.'})
    const p = await readParty(r.resource_id!,c.signal); own(p)
    if (flight !== c || !binding || !auth.privacy.isCurrent(binding.scope) || !allowed()) return
    if (p.revision < r.resource_revision!) throw new Error('Stale resource')
    emit({phase:'succeeded',party:p,saved:null,message:'The server confirmed your party.'})
  }
  async function submit(draft: PartyDraft) {
    if (!allowed() || busy() || state.phase === 'conflict' || state.retryAt > now() || partyId && !state.party) return
    const parsed = partyDraftSchema.safeParse(draft)
    if (!parsed.success || Date.parse(draft.meeting_ends_at) <= now() || !options.places().some(p=>uuid(p.id) && p.id === draft.public_place_id) || state.party && draft.capacity < state.party.member_count) { emit({ message:'Check the fields, capacity, time window, and available place.' }); return }
    const s=auth.privacy.capture(), operation=partyId ? 'updateConcertParty':'createConcertParty', key=(options.newKey ?? (()=>crypto.randomUUID()))()
    if (!uuid(key)) return
    const { state: lifecycle, ...fields }=parsed.data, json={ ...fields, ticket_section:fields.ticket_section || null, ...(partyId ? {expected_revision:state.party!.revision,state:lifecycle}: {}) }
    contractValue(partyId ? 'Domain_party_UpdateParty':'Domain_party_CreateParty',json)
    const b:Binding={scope:s,operation,key,target:partyId ?? null}; binding=b
    const path=`/api/concert/events/${s.eventId}/parties${partyId ? `/${partyId}`:''}`, c=new AbortController(); flight?.abort(); flight=c
    emit({phase:'sending',draft:Object.freeze({...parsed.data}),message:''})
    try { const result = await auth.privacy.run(s,async ({signal})=>(await auth.request<Party | Receipt>({path,method:partyId ? 'PATCH':'POST',access:'protected',errorFamily:'domain',domainOperation:operation,idempotencyKey:key,body:{json},signal,
      decode:(v,ctx)=> { if(ctx.status===202) return receiptDecoder(b,path)(v,ctx); const p=decodeParty(partyId ?? (v as Party)?.party_id,path)(v,ctx); own(p); if(state.party && p.revision <= state.party.revision) throw new Error('Stale write'); return p } })).data,c.signal)
      if(flight!==c || !auth.privacy.isCurrent(s) || !allowed()) return
      if('receipt_id' in result) await acceptReceipt(result,c)
      else emit({phase:'succeeded',party:result,saved:null,message:'The server confirmed your party.'})
    } catch(e) { if(flight===c && auth.privacy.isCurrent(s)) failure(e,true) }
  }
  async function checkStatus() {
    const b=binding; if(!b || !allowed() || !auth.privacy.isCurrent(b.scope) || !['uncertain','pending','reconciling'].includes(state.phase) || state.retryAt>now()) return
    const c=new AbortController(); flight?.abort(); flight=c; const path=`/api/concert/events/${b.scope.eventId}/commands`; emit({phase:'checking',message:''})
    try { const r=await auth.privacy.run(b.scope,async ({signal})=>(await auth.request({path,method:'GET',access:'protected',errorFamily:'domain',domainOperation:'getConcertCommandStatus',query:{operation_id:b.operation,idempotency_key:b.key},signal,decode:receiptDecoder(b,path)})).data,c.signal)
      if(flight===c && auth.privacy.isCurrent(b.scope)) await acceptReceipt(r,c)
    } catch(e) { if(flight===c && auth.privacy.isCurrent(b.scope)) failure(e,true) }
  }
  emit({access:allowed()})
  return { read:()=>state, subscribe(f:()=>void){listeners.add(f); return ()=>{listeners.delete(f)}}, loadLatest, submit, checkStatus,
    places:()=>allowed() ? options.places():[],
    edit(draft:PartyDraft){if(allowed() && !busy()) emit({draft:Object.freeze({...draft})})},
    saveDraft(){if(allowed() && !busy()) emit({saved:Object.freeze({...state.draft}),message:'Draft saved for this active session.'})},
    restoreDraft(){if(allowed() && !busy() && state.saved) emit({draft:Object.freeze({...state.saved}),epoch:state.epoch+1,message:'Draft restored.'})},
    keepDraftWithLatest(){if(allowed() && state.phase==='conflict' && state.latest){emit({party:state.latest,latest:null,phase:'idle',message:'Your draft now uses the reviewed server revision.'})}},
    dispose(){if(disposed)return;reset();disposed=true;cleanup();stopAccess();listeners.clear()},
  }
}
export type PartyAuthoringController = ReturnType<typeof createPartyAuthoring>
