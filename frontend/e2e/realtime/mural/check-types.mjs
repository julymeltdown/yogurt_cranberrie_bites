import {mkdtemp,readFile,writeFile,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {resolve} from 'node:path'
import {execFileSync} from 'node:child_process'
const frontend=resolve(import.meta.dirname,'../../..'), directory=await mkdtemp(resolve(tmpdir(),'mcr-qa05-mural-types-'))
try {
 const {compilerOptions}=JSON.parse(await readFile(resolve(frontend,'tsconfig.json'),'utf8'))
 const config=resolve(directory,'tsconfig.json')
 await writeFile(config,JSON.stringify({compilerOptions:{...compilerOptions,incremental:false,types:[resolve(frontend,'node_modules/vite/client.d.ts')]},files:[resolve(import.meta.dirname,'preview.tsx')]}))
 execFileSync(process.execPath,[resolve(frontend,'node_modules/typescript/bin/tsc'),'--project',config,'--pretty','false'],{cwd:frontend,stdio:'inherit'})
} finally { await rm(directory,{recursive:true,force:true}) }
