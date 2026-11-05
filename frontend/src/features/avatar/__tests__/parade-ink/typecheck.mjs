import { createRequire } from 'node:module'
import { resolve } from 'node:path'
const root = resolve(import.meta.dirname, '../../../../..')
const require = createRequire(resolve(root, 'package.json')), ts = require('typescript')
const config = ts.readConfigFile(resolve(root, 'tsconfig.json'), ts.sys.readFile)
if (config.error) throw Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'))
const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, root)
const roots = [
  'src/app/character-entry/FramedWholeCharacterView.tsx',
  'src/app/account-entry/panels.tsx', 'src/app/social-composition/panels.tsx',
  'src/features/avatar/whole-character/WholeCharacterEditor.tsx',
  'src/app/character-entry/ReviewedCharacterView.tsx', 'src/features/avatar/AvatarEditor.tsx',
  'src/features/avatar/motion-adoption/MotionEditor.tsx', 'src/features/avatar/part-controls/MotionBodyPalette.tsx',
  'src/features/avatar/parade-ink/PortraitFrame.tsx', 'src/features/avatar/parade-ink/AvatarPopover.tsx',
  'src/features/avatar/parade-ink/AvatarCategoryTabs.tsx',
  'src/features/avatar/__tests__/parade-ink/presentation.test.tsx', 'src/features/avatar/__tests__/parade-ink/motion-presentation.test.tsx',
  'src/features/avatar/__tests__/parade-ink/static-portrait.test.tsx', 'src/features/avatar/__tests__/parade-ink/static-editor.test.tsx',
].map(path => resolve(root, path))
const program = ts.createProgram(roots, { ...parsed.options, noEmit: true, incremental: false, composite: false, tsBuildInfoFile: undefined })
const diagnostics = [...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
for (const diagnostic of diagnostics) process.stderr.write(ts.formatDiagnosticsWithColorAndContext([diagnostic], {
  getCurrentDirectory: () => root, getCanonicalFileName: file => file, getNewLine: () => '\n',
}))
console.log(JSON.stringify({ qualification: 'focused changed-source and imported dependency types', rootFiles: roots.length, diagnostics: diagnostics.length }))
process.exitCode = diagnostics.length ? 1 : 0
