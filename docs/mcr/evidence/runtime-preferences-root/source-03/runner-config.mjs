import original from '/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/frontend/vitest.config.ts'
export default {...original,cacheDir:'/private/tmp/mcr-composed-preferences-source03/cache',test:{...original.test,cache:false,setupFiles:[...(original.test?.setupFiles??[]),'/private/tmp/mcr-composed-preferences-source03/setup.mjs']}}
