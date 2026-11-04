// Tests the actual BE-owned, source-qualified module's integration. The tiny
// frontend below is an explicit source fixture, never real IN04 acceptance.
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { mkdtemp, readFile, writeFile, chmod, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import test from 'node:test'
import { buildDeferredAdapter } from './deferred.mjs'
import { browserDependencyIncludes } from './server.mjs'
import { pinnedSource } from './pinned-source.mjs'

const repo = process.env.MCR_IN04_SOURCE_REPO
const backendSource = '2653376e5d8d68f65e05c644e1397a38c6bc02a4'
const backendSha256 = '7d29fee99b5eab4b3e3320ba5a4513f803d6d2b450da33246fa467767deba6ad'
const restore = (name, value) => { if (value === undefined) delete process.env[name]; else process.env[name] = value }

test('canonical browser prebundling excludes the native Tailwind server toolchain and retains lazy Phaser identity', () => {
  const source = pinnedSource(repo, process.env.MCR_IN04_SOURCE_CANDIDATE)
  const manifest = JSON.parse(source.bytes('frontend/package.json'))
  const includes = browserDependencyIncludes(manifest)
  assert(manifest.dependencies['@tailwindcss/vite'] && manifest.dependencies.tailwindcss)
  assert(!includes.includes('@tailwindcss/vite') && !includes.includes('tailwindcss'))
  for (const browserImport of ['phaser', 'react', 'react-dom/client', 'react/jsx-runtime', 'react/jsx-dev-runtime', '@hookform/resolvers/zod']) {
    assert(includes.includes(browserImport), 'Missing canonical browser import: ' + browserImport)
  }
})

test('real source-pinned adapter can be prepared before live frontend binding; import refuses missing input', async () => {
  const directory = await mkdtemp(resolve(tmpdir(), 'mcr-in04-deferred-source-'))
  const saved = process.env.MCR_IN04_FRONTEND_HANDOFF
  delete process.env.MCR_IN04_FRONTEND_HANDOFF
  try {
    const outputPath = resolve(directory, 'runtime.mjs')
    const result = await buildDeferredAdapter({ repo, backendSource, backendSha256, outputPath })
    assert.equal(result.backendSha256, backendSha256)
    const bytes = await readFile(outputPath)
    await assert.rejects(import('data:text/javascript;base64,' + bytes.toString('base64') + '#missing-input'), /Input path must be absolute/)
    await assert.rejects(buildDeferredAdapter({ repo, backendSource, backendSha256: '0'.repeat(64), outputPath: resolve(directory, 'refused.mjs') }), /backend module source differs/)
  } finally { restore('MCR_IN04_FRONTEND_HANDOFF', saved); await rm(directory, { recursive: true }) }
})

test('actual backend module survives data-URL composition and refuses absent managed runtime; live binding stays private', async () => {
  const directory = await mkdtemp(resolve(tmpdir(), 'mcr-in04-real-module-source-'))
  const savedFrontend = process.env.MCR_IN04_FRONTEND_HANDOFF, savedBackend = process.env.MCR_IN04_BACKEND_DESCRIPTOR
  delete process.env.MCR_IN04_BACKEND_DESCRIPTOR
  const candidate = process.env.MCR_IN04_SOURCE_CANDIDATE
  const frontend = { candidate, eventId: '20000000-0000-4000-8000-000000000001', environmentId: '20000000-0000-4000-8000-000000000002',
    settingsSha256: '3'.repeat(64), token: '4'.repeat(64), phaserModulePath: '/@fs/private/synthetic-frontend-fixture/deps/phaser.js' }
  const server = createServer((request, response) => {
    assert.equal(request.headers['x-mcr-in04-observer'], frontend.token)
    const body = JSON.stringify({ ...frontend, expiresAt: new Date(Date.now() + 60000).toISOString(),
      phaserModuleUrl: frontend.origin + frontend.phaserModulePath + '?v=abcdef12' })
    response.writeHead(200, { 'content-length': Buffer.byteLength(body) }); response.end(body)
  })
  await new Promise(done => server.listen(0, '127.0.0.1', done))
  frontend.origin = 'http://127.0.0.1:' + server.address().port
  try {
    const outputPath = resolve(directory, 'runtime.mjs'), handoff = resolve(directory, 'frontend.json')
    await buildDeferredAdapter({ repo, backendSource, backendSha256, outputPath })
    await writeFile(handoff, JSON.stringify(frontend), { mode: 0o600 })
    process.env.MCR_IN04_FRONTEND_HANDOFF = handoff
    const moduleUrl = 'data:text/javascript;base64,' + (await readFile(outputPath)).toString('base64')
    const loaded = (await import(moduleUrl + '#source-fixture')).runtimeQualification
    for (const method of ['inspect', 'observeRestart', 'muralBinding']) assert.equal(typeof loaded[method], 'function')
    await assert.rejects(loaded.inspect({ releaseRevision: candidate, eventId: frontend.eventId, environmentId: frontend.environmentId, origin: frontend.origin }), /IN04_MANAGED_RUNTIME_UNAVAILABLE/)
    await chmod(handoff, 0o644)
    await assert.rejects(import(moduleUrl + '#unsafe-file'), /restricted permissions/)
  } finally {
    restore('MCR_IN04_FRONTEND_HANDOFF', savedFrontend); restore('MCR_IN04_BACKEND_DESCRIPTOR', savedBackend)
    server.closeAllConnections(); await new Promise(done => server.close(done)); await rm(directory, { recursive: true })
  }
})
