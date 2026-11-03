// Test-entry production bundle only. No root-route or product-art qualification.
import { build } from 'vite'
import { createHash } from 'node:crypto'
import { readFile, mkdir, writeFile, mkdtemp } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
const root = fileURLToPath(new URL('../../../../../', import.meta.url))
const output = await mkdtemp('/private/tmp/mcr-fe13-options-production-')
const results = await build({ root, configLoader: 'runner', cacheDir: '/private/tmp/mcr-fe13-options-build-cache',
  build: { outDir: output, emptyOutDir: false, rollupOptions: { input: root+'src/features/avatar/__tests__/option-previews/preview.html' } } })
const bundles = (Array.isArray(results) ? results : [results]).flatMap(result => 'output' in result ? result.output : [])
const hashes = {}
for (const path of ['src/features/avatar/AvatarEditor.tsx', 'src/features/avatar/OptionStrip.tsx', 'src/features/avatar/controller.ts', 'src/features/avatar/avatar-editor.css', 'src/features/avatar/__tests__/option-previews/preview.tsx', 'src/features/avatar/__tests__/option-previews/data.ts']) {
  hashes[path] = createHash('sha256').update(await readFile(root+path)).digest('hex')
}
const evidence = fileURLToPath(new URL('./evidence/', import.meta.url)); await mkdir(evidence, { recursive: true })
await writeFile(evidence+'production.json', JSON.stringify({ result: 'PASS', base: 'e869480395b70b4aae6cfcf50e904b079a59a0e7', syntheticEntryOnly: true, output, hashes,
  bundles: bundles.map(bundle => ({ file: bundle.fileName, sha256: createHash('sha256').update(bundle.type === 'chunk' ? bundle.code : bundle.source).digest('hex') })) }, null, 2)+'\n')
