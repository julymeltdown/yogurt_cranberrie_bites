import assert from 'node:assert/strict'
import {createServer} from 'node:net'
import {statfs,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {resolve} from 'node:path'
export async function requireCaptureSpace(root){const disk=await statfs(root);assert(disk.bavail*disk.bsize>=8*1024**3,'8GiB capture disk floor')}
export async function reservePort(){const reservation=createServer();await new Promise(done=>reservation.listen(0,'127.0.0.1',done));const port=reservation.address().port;await new Promise(done=>reservation.close(done));return port}
export function temporaryCache(kind){return resolve(tmpdir(),`mcr-qa06-${kind}-${process.pid}`)}
export async function clearTemporaryCache(path){assert(path.startsWith(resolve(tmpdir(),'mcr-qa06-')));await rm(path,{recursive:true,force:true})}
