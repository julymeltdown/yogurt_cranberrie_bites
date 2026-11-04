// Local operator control for the owned live browser; JSON action input, sanitized result only.
import {createInterface} from 'node:readline'
import {start} from './session.mjs'
let journey
try { journey=await start({localInputPath:process.env.MCR_IN01_LOCAL_INPUT_PATH}); console.log(JSON.stringify({ready:true})) }
catch { console.log(JSON.stringify({ready:false,code:'SAFE_START_FAILURE'}));process.exitCode=1 }
if(journey){for await(const line of createInterface({input:process.stdin,terminal:false})){try{const command=JSON.parse(line);let result;if(command.action==='script')result=await new Function('j','return (async()=>{'+command.code+'})()')(journey);else result=await journey[command.action](...(command.args??[]));console.log(JSON.stringify({action:command.action,result}));if(command.action==='finish')break}catch{console.log(JSON.stringify({code:'SAFE_CONTROL_FAILURE'}))}}await journey.finish()}
