// Full current frontend read set, no emit/dependency link/install; fresh allocation required.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
if (!dependencies) throw Error('Retained dependencies required')
const require = createRequire(resolve(dependencies, 'package.json')), ts = require('typescript')
const frontend = resolve(import.meta.dirname, '../../../../..')
const config = ts.readConfigFile(resolve(frontend, 'tsconfig.json'), ts.sys.readFile)
const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, frontend)
const program = ts.createProgram([...parsed.fileNames, resolve(dependencies, 'node_modules/vite/client.d.ts')], {
  ...parsed.options, incremental: false, composite: false, tsBuildInfoFile: undefined,
  paths: { react: [resolve(dependencies, 'node_modules/@types/react/index.d.ts')], 'react/*': [resolve(dependencies, 'node_modules/@types/react/*')],
    'react-dom': [resolve(dependencies, 'node_modules/@types/react-dom/index.d.ts')], 'react-dom/*': [resolve(dependencies, 'node_modules/@types/react-dom/*')],
    vite: [resolve(dependencies, 'node_modules/vite/dist/node/index.d.ts')],
    '@vitejs/plugin-react': [resolve(dependencies, 'node_modules/@vitejs/plugin-react/dist/index.d.ts')],
    '@tailwindcss/vite': [resolve(dependencies, 'node_modules/@tailwindcss/vite/dist/index.d.mts')],
    '*': [resolve(dependencies, 'node_modules/*')] }, typeRoots: [resolve(dependencies, 'node_modules/@types')], types: ['node', 'react'],
})
const diagnostics = ts.getPreEmitDiagnostics(program)
for (const diagnostic of diagnostics.slice(0, 30)) process.stderr.write(ts.formatDiagnosticsWithColorAndContext([diagnostic], {
  getCurrentDirectory: () => frontend, getCanonicalFileName: name => name, getNewLine: () => '\n',
}))
console.log(JSON.stringify({ roots: parsed.fileNames.length, transitiveFiles: program.getSourceFiles().length, diagnostics: diagnostics.length }))
process.exitCode = diagnostics.length ? 1 : 0
