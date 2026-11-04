// Allocated SOURCE round only. Source authoring finalization; no assets/native/server outputs.
import { readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { runInNewContext } from 'node:vm'
import ts from 'typescript'

const directory = resolve(import.meta.dirname, '../../../../world/reference-scene/map4-service-v1')
const labelsText = await readFile(resolve(directory, 'annotations.ts'), 'utf8')
const labelsDeclaration = labelsText.slice(labelsText.indexOf('export const map4Labels'), labelsText.indexOf('export function isMap4Presentation')).replace('export const', 'const')
const labels = runInNewContext(ts.transpileModule(labelsDeclaration, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText + '\nmap4Labels')
const groundText = await readFile(resolve(directory, 'paint-source.ts'), 'utf8')
const ground = runInNewContext(ts.transpileModule(groundText.replace('export const', 'const'), { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText + '\nmap4PaintSource')
const style = JSON.parse(await readFile(resolve(directory, 'style-source.json'), 'utf8'))
const hash = 'sha256:' + createHash('sha256').update(JSON.stringify({ labels, style, ground })).digest('hex')
await writeFile(resolve(directory, 'pins.ts'), `// Closed Map-only labels/style/ground digest; checked independently by annotations.test.ts.\nexport const presentationHash = '${hash}'\n`)
process.stdout.write(JSON.stringify({ presentationHash: hash, labels: labels.length, ground: ground.sha256 }) + '\n')
