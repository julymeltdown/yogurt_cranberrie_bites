import assert from 'node:assert/strict'
import { readFile, writeFile, copyFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { writeComparison } from './comparison.mjs'
const output=process.env.MCR_REFERENCE_INTEGRATED_OUTPUT,refresh=process.env.MCR_REFERENCE_INTEGRATED_REFRESH
assert(output?.startsWith('/')&&refresh?.startsWith('/'),'Set absolute evidence and fresh scoped output paths')
const original=JSON.parse(await readFile(resolve(output,'report.json'))),fresh=JSON.parse(await readFile(resolve(refresh,'report.json')))
assert.equal(original.reference.sha256,fresh.reference.sha256);assert.equal(original.clock,fresh.clock)
assert(fresh.cases.length>0&&fresh.cases.every(c=>c.status==='CAPTURED'))
const keys=new Set(fresh.cases.map(c=>`${c.id}:${c.width}`));assert.equal(keys.size,fresh.cases.length)
for(const c of original.cases)c.sourceCandidate??=original.candidate
for(const c of fresh.cases){const index=original.cases.findIndex(old=>old.id===c.id&&old.width===c.width);assert(index>=0);original.cases[index]={...c,sourceCandidate:fresh.candidate};await copyFile(resolve(refresh,c.screenshot),resolve(output,c.screenshot))}
original.findings=[...original.findings.filter(c=>!keys.has(`${c.id}:${c.width}`)),...fresh.findings]
const changedPaths=execFileSync('git',['diff','--name-only',original.productBase,fresh.productBase],{encoding:'utf8'}).trim().split('\n').filter(Boolean)
original.reconciliations??=[];original.reconciliations.push({candidate:fresh.candidate,productBase:fresh.productBase,replaced:[...keys],changedPaths,sourceHashes:fresh.sourceHashes,harnessHashes:fresh.harnessHashes})
await writeFile(resolve(output,`refresh-${fresh.cases[0].id}.json`),JSON.stringify(fresh,null,2)+'\n')
await writeFile(resolve(output,'report.json'),JSON.stringify(original,null,2)+'\n');await writeComparison(output,original)
console.log(JSON.stringify({refreshed:[...keys],candidate:fresh.candidate,status:'PASS'}))
