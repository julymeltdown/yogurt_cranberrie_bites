// Exact frozen Git reads for imports omitted by the closed sparse checkout.
// Never rewrites a fixture, schema, application source, or checkout path.
import {execFileSync} from 'node:child_process'
import {existsSync,readFileSync} from 'node:fs'
import {createHash} from 'node:crypto'
import {resolve,dirname} from 'node:path'
export function sparseSource(repo,pin){
 const git=(...args)=>execFileSync('git',args,{cwd:repo,maxBuffer:32*1024**2})
 const tracked=new Set(git('ls-tree','-r','--name-only',pin,'frontend/src','frontend/e2e','frontend/public','contracts').toString().trim().split('\n').filter(p=>/\.(?:[cm]?tsx?|json|css)$/.test(p)).map(p=>resolve(repo,p)))
 const directories=new Set();for(const path of tracked){let p=dirname(path);while(p.startsWith(repo+'/')){directories.add(p);p=dirname(p)}}
 const cache=new Map(),readset={}
 function read(path){
  if(existsSync(path))return readFileSync(path,'utf8')
  if(!tracked.has(path))return undefined
  if(!cache.has(path)){const relative=path.slice(repo.length+1),bytes=git('show',pin+':'+relative);cache.set(path,bytes.toString());readset[relative]={source:pin,sha256:createHash('sha256').update(bytes).digest('hex'),bytes:bytes.length,reason:'EXACT_TRACKED_SPARSE_INPUT_READ_ONLY'}}
  return cache.get(path)
 }
 return {tracked,directories,read,readset,missing:path=>tracked.has(path)&&!existsSync(path)}
}
