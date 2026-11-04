// Run only under a named fresh functional source allocation.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const repo=resolve(import.meta.dirname,'../../../..'),front=resolve(repo,'frontend'),require=createRequire(resolve(front,'package.json'))
assert(process.env.MCR_FE38_JOB&&process.env.MCR_FE38_TEMP)
const output=resolve(import.meta.dirname,process.env.MCR_FE38_ROUND);await mkdir(output,{recursive:true})
const jobs=[['tests',[resolve(front,'node_modules/vitest/vitest.mjs'),'run','--config','src/features/admin/event/__tests__/current/runner.ts','--configLoader','runner']],
 ['types',[require.resolve('typescript/bin/tsc'),'--project','src/features/admin/event/__tests__/current/tsconfig.json','--noEmit','--incremental','false']],
 ['lint',[resolve(front,'node_modules/oxlint/bin/oxlint'),'src/features/admin/event','src/features/admin/places','src/features/admin/audit']]]
const result={job:process.env.MCR_FE38_JOB,source:'Current original FE38 controller/contract tests and producer regression, scoped strict types/lint; no unrelated matrix',startedAt:new Date().toISOString(),checks:[]}
for(const[name,args]of jobs){const run=spawnSync(process.execPath,args,{cwd:front,encoding:'utf8',env:process.env,timeout:120000,maxBuffer:8*1024**2});await writeFile(resolve(output,name+'.log'),(run.stdout??'')+(run.stderr??'')+(run.error?'\n'+run.error.stack:''));result.checks.push({name,command:[process.execPath,...args],exitCode:run.status,signal:run.signal});if(run.status!==0)break}
result.finishedAt=new Date().toISOString();result.status=result.checks.length===jobs.length&&result.checks.every(c=>c.exitCode===0)?'PASS':'FAIL';await writeFile(resolve(output,'report.json'),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));if(result.status!=='PASS')process.exitCode=1
