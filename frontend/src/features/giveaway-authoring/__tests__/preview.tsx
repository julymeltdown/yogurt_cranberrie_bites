import {createRoot} from 'react-dom/client'
import {GiveawayAuthoring} from '../ui/GiveawayAuthoring'
import {harness,management} from './harness'
import {errorResponse} from '../../party-discovery/__tests__/fixtures/data'
let mode='conflict',revision=1
const h=harness(r=>{const m=management();m.giveaway={...m.giveaway,revision};if(r.method==='PATCH'){if(mode==='conflict'){revision=2;return errorResponse(409,'VERSION_CONFLICT')}throw new Error('lost fixture response')}return Response.json(m)},true)
Object.assign(window,{__FE25__:{mode(v:string){mode=v},read:h.controller.read,logout:()=>h.auth.session.replace(h.auth.session.read().generation,null)}})
createRoot(document.getElementById('root')!).render(<GiveawayAuthoring controller={h.controller} editing/>)
