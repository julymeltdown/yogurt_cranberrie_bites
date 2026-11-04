import {createRequire} from 'node:module'
import fs from 'node:fs'
import path from 'node:path'
import {fileURLToPath} from 'node:url'
const here=path.dirname(fileURLToPath(import.meta.url)),out=path.join(here,'quality-v6')
const require=createRequire(process.env.MCR_PLAYWRIGHT_PACKAGE||path.resolve(here,'../../../package.json'))
const {chromium}=require('@playwright/test')
const browser=await chromium.launch({executablePath:process.env.MCR_BROWSER_EXECUTABLE||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
try{
 const page=await browser.newPage()
 let svg=fs.readFileSync(path.join(out,'ground-layout.svg'),'utf8')
 const source='data:image/png;base64,'+fs.readFileSync(path.join(out,'sources/surface-sheet.png')).toString('base64')
 svg=svg.replaceAll('href="sources/surface-sheet.png"',`href="${source}"`)
 const data=await page.evaluate(async svg=>{
  const image=new Image();image.src='data:image/svg+xml;charset=utf-8,'+encodeURIComponent(svg);await image.decode()
  const canvas=document.createElement('canvas');canvas.width=768;canvas.height=1296;canvas.getContext('2d').drawImage(image,0,0)
  return canvas.toDataURL('image/png')
 },svg)
 fs.writeFileSync(path.join(out,'ground.png'),Buffer.from(data.split(',')[1],'base64'))
 console.log(JSON.stringify({status:'PASS',output:'quality-v6/ground.png',dimensions:[768,1296],source:'ground-layout.svg, unchanged sources/surface-sheet.png',browser:browser.version()}))
}finally{await browser.close()}
