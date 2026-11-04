// Compose verified implementation bytes, not a readiness assertion from config.
import assert from 'node:assert/strict'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { exactKeys, ownedBytes } from './inputs.mjs'
import { sha256 } from './pinned-source.mjs'

export async function prepareAdapter({ backendPath, backendSha256, frontendHandoffPath, outputPath }) {
  const backendBytes = await ownedBytes(backendPath, { maximum: 512 * 1024, hash: backendSha256 })
  assert.match(backendSha256, /^[a-f0-9]{64}$/)
  const frontend = JSON.parse((await ownedBytes(frontendHandoffPath, { privateOnly: true })).toString())
  exactKeys(frontend, ['candidate', 'environmentId', 'eventId', 'origin', 'settingsSha256', 'token', 'phaserModulePath'])
  const implementation = await readFile(new URL('./adapter.mjs', import.meta.url))
  const implementationUrl = 'data:text/javascript;base64,' + implementation.toString('base64')
  const output = Buffer.from(
    'import { composeRuntimeAdapter } from ' + JSON.stringify(implementationUrl) + ';\n'
    + 'export const runtimeQualification = await composeRuntimeAdapter({backendBytes:Buffer.from('
    + JSON.stringify(backendBytes.toString('base64')) + ',"base64"),backendSha256:' + JSON.stringify(backendSha256)
    + ',frontend:' + JSON.stringify(frontend) + '});\n')
  assert(output.length <= 1024 * 1024, 'QA adapter size bound')
  await writeFile(outputPath, output, { flag: 'wx', mode: 0o600 })
  return { adapterSha256: sha256(output), backendSha256, frontendImplementationSha256: sha256(implementation), candidate: frontend.candidate }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [backendPath, backendSha256, frontendHandoffPath, outputPath] = process.argv.slice(2)
  assert([backendPath, backendSha256, frontendHandoffPath, outputPath].every(Boolean),
    'Usage: prepare.mjs REAL_BACKEND_MODULE SHA256 PRIVATE_FRONTEND_HANDOFF NEW_PRIVATE_ADAPTER.mjs')
  process.stdout.write(JSON.stringify(await prepareAdapter({ backendPath, backendSha256, frontendHandoffPath, outputPath })) + '\n')
}
