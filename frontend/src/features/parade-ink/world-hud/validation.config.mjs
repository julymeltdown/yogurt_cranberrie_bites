import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { foundationOverlay } from './foundation-overlay.mjs'
import { aliases } from '../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs'
const require=createRequire(resolve(process.env.MCR_FRONTEND_DEPENDENCIES,'package.json'))
const foundation=foundationOverlay(process.cwd())
export default {
 plugins:[foundation.plugin],
 resolve:{alias:Object.entries({...aliases,'@hookform/resolvers/zod':require.resolve('@hookform/resolvers/zod')}).sort(([a],[b])=>b.length-a.length).map(([find,replacement])=>({find,replacement}))},
 cacheDir:process.env.PI08_CACHE,
 test:{include:['src/features/parade-ink/world-hud/*.test.tsx'],maxWorkers:1,fileParallelism:false},
}
