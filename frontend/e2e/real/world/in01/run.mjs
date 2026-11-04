import {runtimeExit} from './status.mjs'
import {start} from './session.mjs'
import * as flows from './flows.mjs'
let journey
try {
 journey=await start({localInputPath:process.env.MCR_IN01_LOCAL_INPUT_PATH})
 for(const [name,run] of [['login',()=>journey.login()],['world',()=>journey.live()],['movement',()=>journey.move()],['panels',()=>flows.panels(journey)],['theme',()=>flows.theme(journey)],['customization',()=>flows.customization(journey)],['reconnect',()=>flows.reconnect(journey)],['singleGame',()=>flows.singleGame(journey)]]){
  const result=await run();console.log(JSON.stringify({stage:name,result}));if(result?.failed&&['login','world'].includes(name))break
 }
 if(journey.state().checks.authorizedWorld?.status==='PASS'){await journey.shot('world-final-a',0);await journey.shot('world-final-b',1)}
 const state=journey.state();console.log(JSON.stringify({state}));process.exitCode=runtimeExit(state.checks)
}catch{console.log(JSON.stringify({status:'STOPPED',code:'SAFE_GUARD_OR_RUNNER_FAILURE'}));process.exitCode=1}
finally{await journey?.finish()}
