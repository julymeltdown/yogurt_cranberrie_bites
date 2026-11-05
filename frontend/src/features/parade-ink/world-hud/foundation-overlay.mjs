// Read-only PI01 source consumption. No copying, linking or writing peer files.
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { dirname, extname, resolve } from 'node:path'
const digest=bytes=>createHash('sha256').update(bytes).digest('hex')
export function foundationOverlay(frontend) {
 const root=process.env.PI08_FOUNDATION
 if(!root)throw Error('Pinned read-only PI01 source is required')
 const metadata=readFileSync(new URL('./foundation-readset.json',import.meta.url))
 if(digest(metadata)!=='a9b89a8bee524066009722f020cccc869ff506f65a79b35e5d7de289a6f24fd7')throw Error('Immutable PI01 leaf tuple readset changed')
 const rows=JSON.parse(metadata),entries=new Map()
 if(rows.length!==18)throw Error('Exactly18 PI01 leaves required')
 for(const row of rows){
  const bytes=execFileSync('git',['show',row.revision+':'+row.path],{cwd:root,maxBuffer:2*1024**2})
  if(bytes.length!==row.bytes||digest(bytes)!==row.sha256)throw Error('PI01 borrowed readset changed: '+row.path)
  entries.set(resolve(frontend,'..',row.path),{...row,bytes})
 }
 const verify=()=>{for(const entry of entries.values())if(digest(execFileSync('git',['show',entry.revision+':'+entry.path],{cwd:root,maxBuffer:2*1024**2}))!==entry.sha256)throw Error('PI01 immutable postimage mismatch: '+entry.path)}
 const clean=id=>id?.split('?')[0]
 const retainedCSS=new Set(['cozy-identity-backgrounds-v1.css','cozy-community-backgrounds-v1.css'].map(name=>resolve(frontend,'src/shared/styles',name)))
 const cssText=(id,stack=new Set())=>{
  if(stack.has(id))throw Error('Recursive pinned stylesheet import')
  const entry=entries.get(id)
  if(!entry&&!retainedCSS.has(id))throw Error('Unpinned stylesheet import: '+id)
  const next=new Set([...stack,id])
  const text=(entry?.bytes??readFileSync(id)).toString('utf8')
  // Vite's PostCSS import loader reads the filesystem directly, bypassing load().
  // Expand these exact existing imports in memory and preserve their source order.
  return text.replace(/@import\s+["']([^"']+)["'];/g,(_,source)=>cssText(resolve(dirname(id),source),next))
   .replace(/url\(["']?([^"')]+\.ttf)["']?\)/g,(match,source)=>{
    const font=entries.get(resolve(dirname(id),source))
    return font?'url("data:font/ttf;base64,'+font.bytes.toString('base64')+'")':match
   })
 }
 return {
  entries,verify,
  plugin:{name:'pi08-read-only-pi01',enforce:'pre',
   resolveId(source,importer){
    const candidate=source.startsWith('.')&&importer?resolve(dirname(clean(importer)),source):source.startsWith('/')?(entries.has(clean(source))?clean(source):resolve(frontend,'.'+source)):null
    const row=candidate&&entries.get(candidate)
    if(row)return candidate
    return null
   },
   load(id){
    const row=entries.get(clean(id))
    if(!row||!/\.(css|tsx?)$/.test(clean(id)))return null
    return extname(clean(id))==='.css'?cssText(clean(id)):row.bytes.toString('utf8')
   },
  },
 }
}
