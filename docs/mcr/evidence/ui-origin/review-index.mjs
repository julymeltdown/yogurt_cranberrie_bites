import {createRequire} from 'node:module'
import {readFile,writeFile} from 'node:fs/promises'
import {resolve} from 'node:path'
const out=resolve('docs/mcr/evidence/ui-origin'),req=createRequire(resolve(process.env.QA02_NODE_MODULES,'../package.json')),{chromium}=req('@playwright/test')
const {rows}=JSON.parse(await readFile(resolve(out,'states.json'),'utf8'))
const html='<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#ddd;font:13px monospace}.grid{display:grid;grid-template-columns:repeat(8,195px);gap:6px}.item{background:white;overflow:hidden}.item p{height:26px;margin:3px}.item img{width:195px;height:422px;object-fit:cover;object-position:top}</style><div class="grid">'+rows.filter(r=>r.file).map(r=>`<div class="item"><p>${r.id}</p><img src="${r.file}"></div>`).join('')+'</div>'
await writeFile(resolve(out,'state-review.html'),html)
const b=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});try{const p=await b.newPage({viewport:{width:1610,height:1445}});await p.goto('file://'+resolve(out,'state-review.html'),{waitUntil:'networkidle'});await p.screenshot({path:resolve(out,'state-review.jpg'),type:'jpeg',quality:85,fullPage:true})}finally{await b.close()}
