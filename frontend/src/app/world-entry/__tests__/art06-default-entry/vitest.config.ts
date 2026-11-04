import {defineConfig,mergeConfig} from 'vitest/config'
import base from '../../../../../vite.config'
const cache=process.env.MCR_MAP_COMPOSITION_TMP
if(!cache?.startsWith('/'))throw Error('Fresh allocated private TMP required')
export default mergeConfig(base,defineConfig({cacheDir:cache+'/vite',test:{include:[
 ...['composition-overview','consumer','contract','labels','loader','registration'].map(name=>'src/app/world-entry/__tests__/map-presentation-v2/'+name+'.test.'+(name==='consumer'?'tsx':'ts')),
 'src/app/world-entry/__tests__/art06-default-entry/intake.test.ts',
],pool:'threads',maxWorkers:1,fileParallelism:false}}))
