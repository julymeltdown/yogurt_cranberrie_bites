import {runtimeExit} from './status.mjs'
import {start} from './session.mjs'
import * as flows from './flows.mjs'
let journey
try {
 journey=await start({localInputPath:process.env.MCR_IN01_LOCAL_INPUT_PATH})
 for(const [name,run] of [['login',()=>journey.login()],['world',()=>journey.live()],['movement',()=>journey.move()],['panels',()=>flows.panels(journey)],['theme',()=>flows.theme(journey)],['customization',()=>flows.customization(journey)],['sustain',()=>flows.sustain(journey)],['reconnect',()=>flows.reconnect(journey)],['singleGame',()=>flows.singleGame(journey)],['invariants',()=>flows.invariants(journey)]]){
  const result=await run();console.log(JSON.stringify({stage:name,result}));if(result?.failed&&(['login','world'].includes(name)||journey.observers.some(o=>!o.baseline||o.active!==1)))break
 }
 if(journey.state().checks.authorizedWorld?.status==='PASS'&&journey.observers.every(o=>o.baseline&&o.active===1)){await journey.shot('world-final-a',0);await journey.shot('world-final-b',1)}
 const state=journey.state();console.log(JSON.stringify({state}));process.exitCode=runtimeExit(state.checks,state.errors)
}catch{console.log(JSON.stringify({status:'STOPPED',code:'SAFE_GUARD_OR_RUNNER_FAILURE'}));process.exitCode=1}
finally{await journey?.finish();if(journey){const state=journey.state();if(runtimeExit(state.checks,state.errors)!==0)process.exitCode=1}}
