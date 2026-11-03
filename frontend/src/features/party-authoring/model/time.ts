import {utc} from './contract'
/** KST has a fixed UTC+09 offset. Wire values remain strict UTC RFC3339. */
export function kstInput(value:string):string{if(!utc(value))return'';try{return new Date(Date.parse(value)+9*60*60*1000).toISOString().slice(0,19)}catch{return''}}
export function utcFromKst(value:string):string{if(!/^\d{4}-\d\d-\d\dT\d\d:\d\d(?::\d\d)?$/.test(value))return'';const local=`${value.length===16?`${value}:00`:value}Z`;if(!utc(local))return'';try{return new Date(Date.parse(local)-9*60*60*1000).toISOString()}catch{return''}}
const display=new Intl.DateTimeFormat('en-GB',{timeZone:'Asia/Seoul',year:'numeric',month:'short',day:'2-digit',hour:'2-digit',minute:'2-digit',hourCycle:'h23'})
export function kstTime(value:string):string{return utc(value)?`${display.format(new Date(value))} KST`:'Time unavailable'}
