import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { readdirSync } from 'node:fs'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
if (!dependencies) throw Error('Set MCR_FRONTEND_DEPENDENCIES')
const require = createRequire(resolve(dependencies, 'package.json')), ts = require('typescript')
const root = resolve(import.meta.dirname, '../../../../..')
const config = ts.readConfigFile(resolve(root, 'tsconfig.json'), ts.sys.readFile)
const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, root)
const paths = ['src/entities/avatar/runtime/motion-adoption', 'src/world/avatar/motion-adoption', 'src/features/avatar/motion-adoption',
  'src/world/avatar/__tests__/motion-adoption', 'src/features/avatar/__tests__/motion-adoption', 'src/app/character-entry/__tests__/motion-adoption']
const files = process.argv.includes('--application') ? parsed.fileNames : paths.flatMap(p => { try { return readdirSync(resolve(root, p), { recursive: true }).filter(f => /\.(ts|tsx)$/.test(f)).map(f => resolve(root, p, f)) } catch { return [] } })
const program = ts.createProgram([...files, resolve(dependencies, 'node_modules/vite/client.d.ts')], { ...parsed.options, incremental: false, composite: false, tsBuildInfoFile: undefined,
  paths: { vite: [resolve(dependencies, 'node_modules/vite/dist/node/index.d.ts')],
    '@vitejs/plugin-react': [resolve(dependencies, 'node_modules/@vitejs/plugin-react/dist/index.d.ts')],
    '@tailwindcss/vite': [resolve(dependencies, 'node_modules/@tailwindcss/vite/dist/index.d.mts')],
    react: [resolve(dependencies, 'node_modules/@types/react/index.d.ts')], 'react/*': [resolve(dependencies, 'node_modules/@types/react/*')],
    'react-dom': [resolve(dependencies, 'node_modules/@types/react-dom/index.d.ts')],
    'react-dom/*': [resolve(dependencies, 'node_modules/@types/react-dom/*')], '*': [resolve(dependencies, 'node_modules/*')] },
  typeRoots: [resolve(dependencies, 'node_modules/@types')], types: ['node', 'react'] })
const diagnostics = ts.getPreEmitDiagnostics(program)
for (const d of diagnostics.slice(0, 20)) process.stderr.write(ts.formatDiagnosticsWithColorAndContext([d], { getCurrentDirectory: () => root, getCanonicalFileName: f => f, getNewLine: () => '\n' }))
console.log(JSON.stringify({ files: files.length, diagnostics: diagnostics.length }))
process.exitCode = diagnostics.length ? 1 : 0
