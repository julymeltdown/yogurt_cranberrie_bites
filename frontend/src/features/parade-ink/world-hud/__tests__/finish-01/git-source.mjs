// TEST ONLY: immutable canonical source/public reader; three granted HUD postimages override it.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync, existsSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { resolve, dirname, extname, relative } from 'node:path'
export const baseline='7b81bf62d22b48a1ea653a1a84fae199c2b1d837'
export function gitSource(root){
 const frontend=resolve(root,'frontend'),rows=new Map(),cache=new Map(),used=new Map(),served=new Map()
 const owned=new Set(['index.tsx','world-context.tsx','world-hud.css'].map(name=>'frontend/src/features/parade-ink/world-hud/'+name))
 const tree=execFileSync('git',['ls-tree','-r',baseline,'--','frontend','contracts'],{cwd:root,maxBuffer:16*1024**2}).toString()
 for(const line of tree.trim().split('\n')){const [head,path]=line.split('\t'),[mode,type,blob]=head.split(' ');if(type==='blob')rows.set(path,{path,mode,blob})}
 function bytes(path){
  if(owned.has(path))return readFileSync(resolve(root,path))
  const row=rows.get(path);assert(row,'Unpinned Git input '+path)
  if(!cache.has(row.blob))cache.set(row.blob,execFileSync('git',['cat-file','blob',row.blob],{cwd:root,maxBuffer:32*1024**2}))
  const raw=cache.get(row.blob);used.set(path,{...row,bytes:raw.length,sha256:createHash('sha256').update(raw).digest('hex')});return raw
 }
 function css(path,stack=new Set()){
  assert(!stack.has(path),'CSS import cycle');const next=new Set(stack).add(path)
  return bytes(path).toString().replace(/@import\s+["']([^"']+)["']\s*(?:layer\(([^)]+)\)\s*)?;/g,(original,spec,layer)=>{
   if(!spec.startsWith('.'))return original
   const p=relative(root,resolve(dirname(resolve(root,path)),spec));const text=css(p,next);return layer?`@layer ${layer} {\n${text}\n}`:text
  }).replace(/url\(\s*["']?(\.[^"')]+)["']?\s*\)/g,(original,spec)=>{
   const p=relative(root,resolve(dirname(resolve(root,path)),spec));if(!rows.has(p))return original
   const type={'.ttf':'font/ttf','.woff2':'font/woff2','.svg':'image/svg+xml','.png':'image/png'}[extname(p)]
   return type?`url("data:${type};base64,${bytes(p).toString('base64')}")`:original
  })
 }
 const clean=id=>id.split('?')[0]
 const prefix='frontend/src/features/parade-ink/world-hud/__tests__/finish-01/'
 const plugin={name:'pi08-finish-immutable-source',enforce:'pre',resolveId(spec,importer){
  if(!(spec.startsWith('.')||spec.startsWith('/')))return null
  let abs=spec.startsWith('.')&&importer?resolve(dirname(clean(importer)),spec):spec.startsWith('/src/')?resolve(frontend,'.'+spec):spec
  if(abs.startsWith('/@fs/'))abs=abs.slice(4)
  const rel=relative(root,abs)
  if(rel.startsWith(prefix))return existsSync(abs)?abs:null
  const candidates=[rel,...['.ts','.tsx','.mjs','.js','.json','/index.ts','/index.tsx','/index.mjs','/index.js'].map(suffix=>rel+suffix),...(rel.endsWith('.js')?['.ts','.tsx'].map(ext=>rel.slice(0,-3)+ext):[])]
  for(const candidate of candidates)if(rows.has(candidate)||owned.has(candidate))return resolve(root,candidate)
  return null
 },load(id){
  const path=relative(root,clean(id));if(path.startsWith(prefix))return null
  if(!rows.has(path)&&!owned.has(path))return null
  const raw=bytes(path),ext=extname(path)
  if(ext==='.css')return css(path)
  if(['.ts','.tsx','.js','.mjs','.json'].includes(ext))return raw.toString()
  const type={'.ttf':'font/ttf','.woff2':'font/woff2','.png':'image/png','.svg':'image/svg+xml','.jpg':'image/jpeg','.webp':'image/webp'}[ext]
  return type?`export default ${JSON.stringify(`data:${type};base64,${raw.toString('base64')}`)}`:null
 },configureServer(server){server.middlewares.use(async(req,res,next)=>{
  const url=decodeURIComponent((req.url??'').split('?')[0]);const p='frontend/public'+url
  if(rows.has(p)){
   const raw=bytes(p);served.set(url,{...rows.get(p),bytes:raw.length,sha256:createHash('sha256').update(raw).digest('hex')})
   res.setHeader('Content-Type',{'.json':'application/json','.png':'image/png','.svg':'image/svg+xml','.ttf':'font/ttf','.webp':'image/webp','.jpg':'image/jpeg'}[extname(p)]??'application/octet-stream');res.end(raw);return
  }
  if(req.headers.accept?.includes('text/html')){res.setHeader('Content-Type','text/html');res.end(await server.transformIndexHtml(req.url,readFileSync(resolve(frontend,'src/features/parade-ink/world-hud/__tests__/finish-01/preview.html'),'utf8')));return}
  next()
 })}}
 return {plugin,bytes,rows,used,served,owned}
}
