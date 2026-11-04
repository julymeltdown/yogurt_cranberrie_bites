// Use the retained frontend compiler options without adding central configuration.
import {mkdtemp,readFile,writeFile,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {resolve} from 'node:path'
import {execFileSync} from 'node:child_process'

const frontend=resolve(import.meta.dirname,'../../../..')
const directory=await mkdtemp(resolve(tmpdir(),'mcr-qa05-types-'))
try {
 const {compilerOptions}=JSON.parse(await readFile(resolve(frontend,'tsconfig.json'),'utf8'))
 const config=resolve(directory,'tsconfig.json')
 await writeFile(config,JSON.stringify({compilerOptions:{...compilerOptions,types:[resolve(frontend,'node_modules/vite/client.d.ts')],tsBuildInfoFile:resolve(directory,'types.tsbuildinfo')},files:[resolve(import.meta.dirname,'preview.tsx')]}))
 execFileSync(resolve(frontend,'node_modules/.bin/tsc'),['--project',config,'--pretty','false'],{cwd:frontend,stdio:'inherit'})
} finally { await rm(directory,{recursive:true,force:true}) }
