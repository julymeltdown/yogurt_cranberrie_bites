import {execFileSync, spawnSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import {mkdir,writeFile} from 'node:fs/promises'
import {resolve,dirname} from 'node:path'
import assert from 'node:assert/strict'
const root=resolve(import.meta.dirname,'../../../..'), front=resolve(root,'frontend'), temporary=process.env.MCR_VENU_TMP
assert(temporary?.startsWith('/private/tmp/mcr-paradise-city-map-'))
const producer='4f94914d06d54c3b465930cc23d40be91a1255cb', prefix='frontend/src/features/paradise-city-map/spatial/'
const expected={
 'contract.ts':'b75f7cf748949a1a858626fe098f3d1b0dd22e65eeea31d551b37fa309745a42',
 'image-resource.ts':'6d5e0f7c607d8ae17f03dd3030aa52e7c57d2f0ff1377d9384c6bed2554aba18',
 'model.ts':'370023d8a25d135fce56969c5b5e2e8a20bfdd17e92eb0ab24da04b8aae219bd',
 'registration.ts':'75c57f43a81ecaeab1cd9b0dad86329744516285501137fa5d4abb8ff25fe314',
 'viewport.ts':'19489edc14c739050c3f4c6a8b32d76156f059f7620eeaa155148ff4a5a193f9',
}
const overlay=resolve(temporary,'type-inputs'), inputs=[]
for(const [name,sha256] of Object.entries(expected)) {
 const path=prefix+name, bytes=execFileSync('git',['show',`${producer}:${path}`],{cwd:root})
 assert.equal(createHash('sha256').update(bytes).digest('hex'),sha256)
 const target=resolve(overlay,path.slice('frontend/src/'.length)); await mkdir(dirname(target),{recursive:true}); await writeFile(target,bytes,{mode:0o444})
 inputs.push({path,sha256,bytes:bytes.length,privateOverlay:target})
}
const config=resolve(temporary,'full-ordinary-tsconfig.json')
await writeFile(config,JSON.stringify({extends:resolve(front,'tsconfig.json'),include:[resolve(front,'src'),resolve(front,'vite.config.ts'),resolve(front,'e2e/venue-map')],compilerOptions:{incremental:false,rootDirs:[resolve(front,'src'),overlay],typeRoots:[resolve(front,'node_modules/@types'),resolve(front,'node_modules')],paths:{zod:[resolve(front,'node_modules/zod')]}}},null,2))
await writeFile(resolve(process.env.MCR_VENU_OUTPUT,'type-input-receipt.json'),JSON.stringify({producer,inputs,classification:'ACTUAL_SPATIAL_PRODUCER_INPUT_NOT_PRODUCER_ACCEPTANCE',ordinaryProject:resolve(front,'tsconfig.json'),changes:['rootDirs: five exact immutable Git producer modules in read-only private TMP','typeRoots: original explicit vite/client anchored to retained frontend dependencies; inherited types unchanged','zod: existing read-only package resolution','incremental:false','include: all ordinary source/config plus own actual-App fixture, no exclusions']},null,2)+'\n')
const result=spawnSync(process.execPath,[resolve(front,'node_modules/typescript/bin/tsc'),'-p',config,'--pretty','false'],{cwd:front,encoding:'utf8'})
process.stdout.write(result.stdout??'');process.stderr.write(result.stderr??'');process.exit(result.status??1)
