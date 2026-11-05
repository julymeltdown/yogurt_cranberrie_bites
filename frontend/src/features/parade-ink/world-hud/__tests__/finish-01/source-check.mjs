// Meaningful CSS syntax and retained-behavior boundary check; geometry is tested in the real browser.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { baseline } from './git-source.mjs'
const frontend=process.cwd(),root=resolve(frontend,'..'),folder='frontend/src/features/parade-ink/world-hud/'
const pins=JSON.parse(readFileSync((process.env.PI08_INPUT_FILE??resolve(root,'docs/mcr/evidence/pi08-finish-01/native01-inputs.json'))))
for(const row of pins.implementationAndDrivers){const raw=readFileSync(resolve(root,row.path));assert.equal(raw.length,row.bytes,row.path);assert.equal(createHash('sha256').update(raw).digest('hex'),row.sha256,row.path)}
for(const name of ['index.tsx','world-context.tsx'])assert.deepEqual(readFileSync(resolve(root,folder+name)),execFileSync('git',['show',baseline+':'+folder+name],{cwd:root}),name+' authorized behavior unchanged')
const require=createRequire(import.meta.resolve('vite')),postcss=require('postcss')
const path=resolve(root,folder+'world-hud.css'),ast=postcss.parse(readFileSync(path,'utf8'),{from:path});let rules=0,declarations=0
ast.walkRules(()=>rules++);ast.walkDecls(()=>declarations++);assert(rules>0&&declarations>0)
console.log(JSON.stringify({status:'PASS',CSSParsed:true,rules,declarations,typedBehavior:'Two JSX leaves byte-identical canonical baseline; prior ten qualified HUD cases retained, no replay',scope:'CSS geometry only; actual source-pinned default-App comparisons required'}))
