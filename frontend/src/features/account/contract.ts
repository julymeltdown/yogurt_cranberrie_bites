import { z } from 'zod'
import { freeze, uuid } from '../safety/contract'
const timestamp=z.iso.datetime({offset:true})
export const deletionTargets=['PROFILE','FRIEND','POST','NOTIFICATION'] as const
const step=z.object({target:z.enum(deletionTargets),status:z.enum(['PENDING','IN_PROGRESS','RETRY','SUCCEEDED','DEAD_LETTERED']),attemptCount:z.number().int().safe().min(0),nextAttemptAt:timestamp,lastError:z.string().nullable(),completedAt:timestamp.nullable()})
// Inherited Auth schemas are open; z.object strips extensions instead of exposing them.
const saga=z.object({id:uuid,userId:uuid,status:z.enum(['IN_PROGRESS','COMPLETED','DEAD_LETTERED']),retentionUntil:timestamp,createdAt:timestamp,updatedAt:timestamp,completedAt:timestamp.nullable(),steps:z.array(step).max(4)})
const accepted=z.object({userId:uuid,deleted:z.boolean(),sagaId:uuid,saga})
export interface DeletionBinding{readonly actorId:string;readonly sagaId:string|null;readonly updatedAt:number|null}
export interface DeletionProgress{readonly state:'pending'|'partial-failure'|'complete';readonly steps:readonly {readonly target:typeof deletionTargets[number];readonly state:z.infer<typeof step>['status']}[]}
export function decodeDeletion(value:unknown,status:200|202,binding:DeletionBinding,now:number){
 const envelope=status===202?accepted.parse(value):null,s=envelope?.saga??saga.parse(value)
 if(envelope&&s.status==='COMPLETED'&&!envelope.deleted)throw new Error('Account not disabled')
 if(s.userId!==binding.actorId||envelope&&(envelope.userId!==binding.actorId||envelope.sagaId!==s.id)||binding.sagaId!==null&&s.id!==binding.sagaId)throw new Error('Changed deletion identity')
 const created=Date.parse(s.createdAt),updated=Date.parse(s.updatedAt)
 if(created>updated||updated>now||binding.updatedAt!==null&&updated<binding.updatedAt||s.steps.length!==4||new Set(s.steps.map(s=>s.target)).size!==4)throw new Error('Invalid deletion progress')
 const finished=s.steps.every(step=>step.status==='SUCCEEDED'&&step.completedAt!==null&&Date.parse(step.completedAt)>=created&&Date.parse(step.completedAt)<=updated)
 if(s.status==='COMPLETED'&&(!finished||s.completedAt===null||Date.parse(s.completedAt)<created||Date.parse(s.completedAt)>updated))throw new Error('Incomplete deletion')
 if(s.status!=='COMPLETED'&&s.completedAt!==null)throw new Error('Invalid pending deletion')
 const progress:DeletionProgress=freeze({state:s.status==='COMPLETED'?'complete':s.status==='DEAD_LETTERED'||s.steps.some(s=>s.status==='DEAD_LETTERED')?'partial-failure':'pending',steps:s.steps.map(s=>({target:s.target,state:s.status}))})
 return freeze({binding:{actorId:binding.actorId,sagaId:s.id,updatedAt:updated},progress})
}
