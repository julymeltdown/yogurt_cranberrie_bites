// Only newly changed PI08 source validation; no old functional/browser matrix.
import { createRequire } from 'node:module'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { spawn, execFileSync } from 'node:child_process'
import { foundationOverlay } from './foundation-overlay.mjs'
const frontend=resolve(import.meta.dirname,'../../../..'),deps=process.env.MCR_FRONTEND_DEPENDENCIES
const out=process.env.PI08_OUTPUT
if(!deps||!out)throw Error('Finite source allocation/output required')
const require=createRequire(resolve(deps,'package.json')),ts=require('typescript')
const foundation=foundationOverlay(frontend)
await mkdir(out,{recursive:true})
const report={status:'FAIL',qualification:'PI08_SOURCE_ONLY',checks:[],foundation:'b5b44e47531e3b75febc5ee5cdd6a11b1648779c',foundationQualification:'Explicit one-file CSS correction; native acceptance not inferred'}
async function command(label,args){
 const code=await new Promise((done,reject)=>{const child=spawn(process.execPath,args,{cwd:frontend,env:process.env,stdio:'inherit'});child.once('error',reject);child.once('exit',code=>done(code))})
 report.checks.push({label,exit:code});return code===0
}
try{
 const testPath='frontend/src/features/parade-ink/world-hud/world-hud.test.tsx'
 const prior=execFileSync('git',['show','df5445fb5ccaa68bf7e9bdd3895ea37221765a32:'+testPath],{cwd:resolve(frontend,'..'),encoding:'utf8'})
 const current=await readFile(resolve(frontend,'..',testPath),'utf8')
 const emit=source=>ts.transpileModule(source,{compilerOptions:{jsx:ts.JsxEmit.ReactJSX,target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText
 const unit=emit(prior)===emit(current)
 report.checks.push({label:'retained10passingSOURCE02 cases; fixture type corrections emit identical JavaScript',exit:unit?0:1,source:'df5445fb5ccaa68bf7e9bdd3895ea37221765a32'})
 if(!unit)throw Error('Retained test runtime changed; a fresh unit allocation is required')
 const config=ts.readConfigFile(resolve(frontend,'tsconfig.json'),ts.sys.readFile)
 const parsed=ts.parseJsonConfigFileContent(config.config,ts.sys,frontend)
 const options={...parsed.options,noEmit:true,incremental:false,paths:{...parsed.options.paths,zod:[resolve(deps,'node_modules/zod')]}};delete options.tsBuildInfoFile
 const host=ts.createCompilerHost(options),read=host.readFile.bind(host),exists=host.fileExists.bind(host)
 host.readFile=file=>foundation.entries.get(resolve(file))?.bytes.toString('utf8')??read(file)
 host.fileExists=file=>foundation.entries.has(resolve(file))||exists(file)
 const program=ts.createProgram(parsed.fileNames,options,host)
 const diagnostics=ts.getPreEmitDiagnostics(program)
 const formatted=ts.formatDiagnosticsWithColorAndContext(diagnostics,{getCanonicalFileName:x=>x,getCurrentDirectory:()=>frontend,getNewLine:()=> '\n'})
 await writeFile(resolve(out,'types.log'),formatted)
 report.checks.push({label:'full nonincremental frontend types with pinned PI01 read-only postimages',errors:diagnostics.length,exit:diagnostics.length?1:0})
 let syntax=true
 for(const entry of ['source.mjs','foundation-overlay.mjs'])syntax=(await command('syntax '+entry,['--check',resolve(import.meta.dirname,entry)]))&&syntax
 const lint=await command('changed fixture type metadata and source driver lint; unchanged UI/caller lint retained SOURCE02',[resolve(deps,'node_modules/oxlint/bin/oxlint'),'--deny-warnings','src/features/parade-ink/world-hud/source.mjs','src/features/parade-ink/world-hud/world-hud.test.tsx','src/features/parade-ink/world-hud/foundation-overlay.mjs'])
 foundation.verify()
 report.status=unit&&!diagnostics.length&&syntax&&lint?'PASS':'FAIL'
}finally{await writeFile(resolve(out,'source.json'),JSON.stringify(report,null,2)+'\n')}
if(report.status!=='PASS')process.exitCode=1
