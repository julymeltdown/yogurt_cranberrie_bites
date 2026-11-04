// Named bounded source job only. Types/lint/syntax for additive harness; no baseline replay.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const repo=resolve(import.meta.dirname,'../../../..'),front=resolve(repo,'frontend'),require=createRequire(resolve(front,'package.json'))
assert(process.env.QA07_JOB&&process.env.QA07_TEMP)
const out=resolve(import.meta.dirname,process.env.QA07_ROUND);await mkdir(out,{recursive:true})
const jobs=[['types',[require.resolve('typescript/bin/tsc'),'-p','e2e/qa07-input-delta/tsconfig.json','--noEmit','--incremental','false']],['lint',[resolve(front,'node_modules/oxlint/bin/oxlint'),'e2e/qa07-input-delta']],['syntax',['--check','e2e/qa07-input-delta/native.mjs']],['plan',['PLAN']]]
const result={job:process.env.QA07_JOB,checks:[],started:new Date().toISOString()}
for(const[name,args]of jobs){const run=name==='plan'?spawnSync('python3',['scripts/mcr/check-plan.py'],{cwd:repo,encoding:'utf8',timeout:30000}):spawnSync(process.execPath,args,{cwd:front,encoding:'utf8',env:process.env,timeout:90000,maxBuffer:4*1024**2});await writeFile(resolve(out,name+'.log'),(run.stdout??'')+(run.stderr??'')+(run.error?'\n'+run.error.stack:''));result.checks.push({name,command:name==='plan'?['python3','scripts/mcr/check-plan.py']:[process.execPath,...args],exit:run.status,signal:run.signal});if(run.status!==0)break}
result.finished=new Date().toISOString();result.status=result.checks.length===jobs.length&&result.checks.every(c=>c.exit===0)?'PASS':'FAIL';await writeFile(resolve(out,'report.json'),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));if(result.status!=='PASS')process.exitCode=1
