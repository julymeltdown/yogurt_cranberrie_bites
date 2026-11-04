import assert from 'node:assert/strict'
import { readFile,writeFile,mkdir,statfs } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { chromium } from '@playwright/test'
assert(process.env.MCR_COZY_EXPORT_JOB,'Fresh named export allocation required')
const input='/Users/lhs/Desktop/mcr-worktrees/art-map-cozy-v1/frontend/public/world/reference-scene-v2/map-cozy-v1/ground.svg'
const output=new URL('../../../../../../public/world/reference-scene-v2/map-cozy-v1/parade/ground.png',import.meta.url)
const raw=await readFile(input),sha=b=>createHash('sha256').update(b).digest('hex')
assert.equal(sha(raw),'e52f9acdaf4bc491664125585776ae42fe34848a59f5edc0205880095a19de40')
const d=await statfs('.');assert(d.bavail*d.bsize>=16*1024**3)
let browser
try{browser=await chromium.launch({channel:'chrome',headless:true});const context=await browser.newContext({viewport:{width:768,height:1296},serviceWorkers:'block'});await context.route('http://**/*',r=>r.abort());await context.route('https://**/*',r=>r.abort());const page=await context.newPage();await page.goto('about:blank');const data=await page.evaluate(async svg=>{const blob=new Blob([svg],{type:'image/svg+xml'}),url=URL.createObjectURL(blob),image=new Image();try{image.src=url;await image.decode();if(image.naturalWidth!==768||image.naturalHeight!==1296)throw Error('Wrong source dimensions');const canvas=document.createElement('canvas');canvas.width=768;canvas.height=1296;canvas.getContext('2d').drawImage(image,0,0);return canvas.toDataURL('image/png').split(',')[1]}finally{URL.revokeObjectURL(url)}},raw.toString());const png=Buffer.from(data,'base64');assert(png.length<=896034,'Closed2MiB scenery transfer budget');assert.equal(png.readUInt32BE(16),768);assert.equal(png.readUInt32BE(20),1296);await mkdir(new URL('.',output),{recursive:true});await writeFile(output,png,{flag:'wx'});console.log(JSON.stringify({job:process.env.MCR_COZY_EXPORT_JOB,input,inputSha256:sha(raw),output:output.pathname,bytes:png.length,sha256:sha(png),dimensions:[768,1296],browser:browser.version(),time:new Date().toISOString(),result:'PASS'}));await context.close()}finally{await browser?.close()}
