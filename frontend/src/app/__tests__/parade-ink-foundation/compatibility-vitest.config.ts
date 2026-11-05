import { defineConfig } from 'vitest/config'
export default defineConfig({cacheDir:`${process.env.TMPDIR}/compat-cache`,test:{
  include:['src/app/__tests__/reference-frame-footer/footer.test.tsx','src/app/reference-chrome-v2/chrome.test.tsx'],
  maxWorkers:1,fileParallelism:false,pool:'forks',testTimeout:10000,
}})
