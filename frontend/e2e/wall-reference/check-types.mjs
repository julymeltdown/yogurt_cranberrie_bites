// No-emit semantic check against the same committed feature bytes served by run.mjs.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import ts from 'typescript'
const root = fileURLToPath(new URL('../../../', import.meta.url)), pin = process.env.WALL_REFERENCE_AUTHOR
assert(pin && /^[0-9a-f]{40}$/.test(pin), 'Final author commit required')
const paths = ['frontend/src/features/mural/editor/MuralEditor.tsx', 'frontend/src/features/mural/editor/editor.css', 'frontend/src/features/mural/sync/MuralSyncPanel.tsx']
const overrides = new Map(paths.map(path => [resolve(root, path), execFileSync('git', ['show', `${pin}:${path}`], { cwd: root, encoding: 'utf8' })]))
const options = { noEmit:true, target:ts.ScriptTarget.ES2022, lib:['lib.es2022.d.ts','lib.dom.d.ts','lib.dom.iterable.d.ts'], module:ts.ModuleKind.ESNext,
  moduleResolution:ts.ModuleResolutionKind.Bundler, jsx:ts.JsxEmit.ReactJSX, strict:true, noUncheckedIndexedAccess:true, exactOptionalPropertyTypes:true,
  isolatedModules:true, verbatimModuleSyntax:true, resolveJsonModule:true, skipLibCheck:true, noUnusedLocals:true, noUnusedParameters:true,
  types:['node'], typeRoots:[resolve(root,'frontend/node_modules/@types')] }
const host=ts.createCompilerHost(options), original=host.readFile.bind(host), loaded=new Set()
host.readFile=path=>{if(overrides.has(path)){loaded.add(path);return overrides.get(path)}return original(path)}
const program=ts.createProgram([resolve(root,'frontend/e2e/wall-reference/fixture.tsx'),resolve(root,'frontend/node_modules/vite/client.d.ts')],options,host)
const diagnostics=ts.getPreEmitDiagnostics(program)
if(diagnostics.length){console.error(ts.formatDiagnosticsWithColorAndContext(diagnostics,{getCanonicalFileName:f=>f,getCurrentDirectory:()=>root,getNewLine:()=> '\n'}));process.exitCode=1}
else{assert(loaded.has(resolve(root,paths[0])) && loaded.has(resolve(root,paths[2])));console.log(JSON.stringify({scope:'owned fixture + readonly final feature dependency',pin,status:'PASS',noEmit:true,loadedPinnedTypeScriptPaths:[...loaded]}))}
