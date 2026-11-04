// Prepare the real source-pinned adapter before a service window. Private live
// binding is read only when QA imports the artifact, never invented at build time.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { sha256 } from './pinned-source.mjs'

export async function buildDeferredAdapter({ repo, backendSource, backendSha256, outputPath }) {
  assert.match(backendSource, /^[a-f0-9]{40}$/)
  assert.match(backendSha256, /^[a-f0-9]{64}$/)
  const backend = execFileSync('git', ['show', backendSource + ':backend/scripts/mcr/in04_wall_runtime/runtime.mjs'], { cwd: repo, maxBuffer: 512 * 1024 })
  assert.equal(sha256(backend), backendSha256, 'Genuine backend module source differs')
  const [implementation, reader] = await Promise.all(['adapter.mjs', 'private-file.mjs'].map(name => readFile(new URL(name, import.meta.url))))
  const dataUrl = bytes => 'data:text/javascript;base64,' + bytes.toString('base64')
  const output = Buffer.from([
    '// Generated from qualified implementation bytes; contains no live identity or ready proof.',
    'import assert from "node:assert/strict";',
    'import { composeRuntimeAdapter } from ' + JSON.stringify(dataUrl(implementation)) + ';',
    'import { ownedBytes } from ' + JSON.stringify(dataUrl(reader)) + ';',
    'const frontend = JSON.parse((await ownedBytes(process.env.MCR_IN04_FRONTEND_HANDOFF,{privateOnly:true})).toString());',
    'assert.deepEqual(Object.keys(frontend).sort(),["candidate","environmentId","eventId","origin","settingsSha256","token","phaserModulePath"].sort());',
    'export const runtimeQualification = await composeRuntimeAdapter({backendBytes:Buffer.from(' + JSON.stringify(backend.toString('base64')) + ',"base64"),backendSha256:' + JSON.stringify(backendSha256) + ',frontend});',
    '',
  ].join('\n'))
  assert(output.length <= 1024 * 1024, 'QA adapter size bound')
  await writeFile(outputPath, output, { flag: 'wx', mode: 0o600 })
  return { adapterSha256: sha256(output), backendSource, backendSha256,
    frontendImplementationSha256: sha256(implementation), privateReaderSha256: sha256(reader), bytes: output.length }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [repo, backendSource, backendSha256, outputPath] = process.argv.slice(2)
  assert([repo, backendSource, backendSha256, outputPath].every(Boolean),
    'Usage: deferred.mjs REPO EXACT_BACKEND_SOURCE SHA256 NEW_PRIVATE_ADAPTER.mjs')
  process.stdout.write(JSON.stringify(await buildDeferredAdapter({ repo, backendSource, backendSha256, outputPath })) + '\n')
}
