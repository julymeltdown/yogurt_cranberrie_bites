import { execFileSync } from 'node:child_process'
import { writeFile, unlink } from 'node:fs/promises'
import { resolve } from 'node:path'
import { startVitest } from 'vitest/node'
const path = resolve('src/features/media/model/__tests__/http-baseline.fixture.ts')
let source = execFileSync('git', ['show','48458d597af73bd695c8d5813b123c6a96e55321:frontend/src/shared/api/http.ts'], {encoding:'utf8'})
source = source.replaceAll("from './", "from '../../../../shared/api/")
await writeFile(path, source)
let ctx
try {
  ctx = await startVitest('test', ['src/features/auth/__tests__/enrollment.test.ts'], {
    watch:false, plugins:[{name:'exact-pre-FE04-baseline',enforce:'pre',resolveId(id){
      if(id.endsWith('/shared/api/http') || id.endsWith('/shared/api/http.ts')) return path
    }}],
  })
  if(ctx.state.getFiles().some(f=>f.result?.state==='fail')) process.exitCode=1
} finally { await ctx?.close(); await unlink(path) }
