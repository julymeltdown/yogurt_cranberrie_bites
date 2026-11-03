import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import { build } from 'vite'
const root = fileURLToPath(new URL('../../../../../../', import.meta.url))
const inputs = {
  preview: 'src/features/intro/__tests__/evidence/unicode-length/preview.html',
  text: 'src/shared/validation/text.ts', tags: 'src/shared/validation/tags.ts',
  contract: 'src/entities/intro/model/contract.ts', policy: 'src/entities/intro/model/policy.ts',
  age: 'src/entities/intro/model/age.ts', errors: 'src/entities/intro/model/errors.ts',
}
const outputs = await build({ root, configLoader: 'runner', cacheDir: '/private/tmp/mcr-fe03-build-cache', build: {
  outDir: '/private/tmp/mcr-fe03-production', emptyOutDir: true,
  rollupOptions: { input: Object.fromEntries(Object.entries(inputs).map(([key, path]) => [key, resolve(root, path)])), preserveEntrySignatures: 'strict' },
} })
const hashes = Object.fromEntries(await Promise.all(Object.values(inputs).map(async path => [path, createHash('sha256').update(await readFile(resolve(root, path))).digest('hex')])))
await writeFile(new URL('./build-report.json', import.meta.url), JSON.stringify({ result: 'PASS', node: process.version, scope: 'Explicit helper entries and actual editor/controller synthetic fixture; no root shell or live acceptance', hashes, outputs: (Array.isArray(outputs) ? outputs : [outputs]).flatMap(result => result.output.map(file => file.fileName)) }, null, 2)+'\n')
