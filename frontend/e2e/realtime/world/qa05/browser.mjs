// Dedicated temporary Chrome profile. noDefaults preserves actual tab visibility;
// ordinary Playwright contexts deliberately force visible/focused-page emulation.
import {spawn} from 'node:child_process'
import {mkdtemp,readFile,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {chromium} from '@playwright/test'

export async function nativeBrowser() {
 const profile=await mkdtemp(tmpdir()+'/mcr-qa05-browser-')
 const process=spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',[
  '--remote-debugging-port=0','--user-data-dir='+profile,'--no-first-run','--no-default-browser-check','about:blank',
 ],{stdio:'ignore'})
 let browser
 const close=async()=>{
  await browser?.close().catch(()=>{})
  if(process.exitCode===null&&process.signalCode===null){process.kill();for(let i=0;i<30&&process.exitCode===null&&process.signalCode===null;i++)await new Promise(r=>setTimeout(r,100));if(process.exitCode===null&&process.signalCode===null)process.kill('SIGKILL')}
  await rm(profile,{recursive:true,force:true})
 }
 try{
  let port
  for(let i=0;i<60;i++){try{port=(await readFile(profile+'/DevToolsActivePort','utf8')).split('\n')[0];break}catch{await new Promise(r=>setTimeout(r,100))}}
  if(!port)throw Error('Dedicated native Chrome did not expose its debugging port')
  browser=await chromium.connectOverCDP('http://127.0.0.1:'+port,{noDefaults:true})
  return {browser,context:browser.contexts()[0],close}
 }catch(e){await close();throw e}
}
