// Root-owned build-input identity. This is not an emitted-bundle hash, a server
// recognition decision, or an authorization grant. No package-version fallback.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync, spawn } from 'node:child_process'
import { createReadStream } from 'node:fs'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { sha256 } from './pinned-source.mjs'
import { compiledClockIdentity } from './community-clock-input.mjs'

// Explicit application-release declaration authorized by the deployment owner.
export const ROOT_APPLICATION_VERSION = '0.1.0'
const MAX_INPUT_BYTES = 1024 ** 3

export function canonicalJson(value) {
  function ordered(item) {
    if (Array.isArray(item)) return item.map(ordered)
    if (item && typeof item === 'object') return Object.fromEntries(Object.keys(item).sort().map(key => [key, ordered(item[key])]))
    assert(item === null || typeof item === 'string' || typeof item === 'boolean' || typeof item === 'number' && Number.isFinite(item))
    return item
  }
  return JSON.stringify(ordered(value)) + '\n'
}

function selectedInput(path) {
  if (path.startsWith('frontend/public/')) return true
  if (path.startsWith('frontend/src/')) return !path.includes('/__tests__/') && !/\.(?:test|spec)\.[^/]+$/.test(path)
  return ['frontend/index.html', 'frontend/package.json', 'frontend/pnpm-lock.yaml', 'frontend/vite.config.ts'].includes(path)
    || /^frontend\/tsconfig(?:\.[^/]+)?\.json$/.test(path)
}

// Stream exact Git blob bytes; do not copy the large public art trees to disk or
// accumulate them in memory. Duplicate blobs are read once, paths remain distinct.
async function hashBlobs(repo, entries) {
  const unique = [...new Map(entries.map(entry => [entry.gitBlob, entry])).values()]
  const child = spawn('git', ['cat-file', '--batch'], { cwd: repo, stdio: ['pipe', 'pipe', 'pipe'] })
  const done = new Promise(resolveDone => {
    child.once('error', error => resolveDone({ error }))
    child.once('close', code => resolveDone({ code }))
  })
  let stderr = '', pending = Buffer.alloc(0), index = 0, remaining = 0, digest = null, separator = false
  const hashes = new Map()
  child.stderr.on('data', data => { stderr += data.toString(); if (stderr.length > 8192) child.kill('SIGKILL') })
  child.stdin.on('error', () => { /* Completion and missing blobs fail below. */ })
  const timer = setTimeout(() => child.kill('SIGKILL'), 120000)
  timer.unref()
  child.stdin.end(unique.map(entry => entry.gitBlob + '\n').join(''))
  try {
    for await (const chunk of child.stdout) {
      pending = Buffer.concat([pending, chunk])
      for (;;) {
        if (separator) {
          if (!pending.length) break
          assert.equal(pending[0], 10, 'Git blob terminator differs')
          pending = pending.subarray(1); separator = false; index++
        }
        if (!digest) {
          if (!pending.length) break
          const end = pending.indexOf(10)
          if (end === -1) { assert(pending.length < 160); break }
          const expected = unique[index]
          assert(expected, 'Unexpected Git output')
          assert.equal(pending.subarray(0, end).toString(), expected.gitBlob + ' blob ' + expected.bytes)
          pending = pending.subarray(end + 1); remaining = expected.bytes; digest = createHash('sha256')
        }
        const count = Math.min(remaining, pending.length)
        digest.update(pending.subarray(0, count)); remaining -= count; pending = pending.subarray(count)
        if (remaining) break
        hashes.set(unique[index].gitBlob, digest.digest('hex')); digest = null; separator = true
      }
    }
    const result = await done
    assert(!result.error && result.code === 0 && stderr.length <= 8192, 'Git source read failed')
    assert(index === unique.length && !pending.length && !digest && !separator, 'Incomplete Git source read')
    return entries.map(entry => ({ ...entry, sha256: hashes.get(entry.gitBlob) }))
  } finally { clearTimeout(timer); child.kill('SIGTERM'); await done }
}

async function fileHash(path) {
  const digest = createHash('sha256')
  for await (const chunk of createReadStream(path)) digest.update(chunk)
  return digest.digest('hex')
}

export async function buildApplicationRelease({ repo, candidate, dependencies, mode, communityClockInput = null }) {
  assert.match(candidate, /^[a-f0-9]{40}$/)
  assert.equal(candidate.length, 40)
  assert(['vite-serve', 'vite-build'].includes(mode), 'Explicit build mode required')
  const git = (...args) => execFileSync('git', args, { cwd: repo, maxBuffer: 8 * 1024 ** 2 })
  assert.equal(git('rev-parse', candidate + '^{commit}').toString().trim(), candidate)
  git('merge-base', '--is-ancestor', candidate, 'refs/remotes/origin/master')
  for (const path of ['package.json', 'pnpm-lock.yaml']) {
    assert((await readFile(resolve(dependencies, '..', path))).equals(git('show', candidate + ':frontend/' + path)), 'Dependency declaration differs')
  }
  const rows = git('ls-tree', '-z', '-r', '-l', candidate, '--', 'frontend').toString().split('\0').filter(Boolean)
  const entries = []
  for (const row of rows) {
    const tab = row.indexOf('\t'), path = row.slice(tab + 1)
    if (!selectedInput(path)) continue
    const match = /^(100644|100755) blob ([a-f0-9]{40})\s+(\d+)$/.exec(row.slice(0, tab))
    assert(match && ![...path].some(character => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127), 'Only regular bounded application inputs supported')
    const bytes = Number(match[3]); assert(bytes <= 32 * 1024 ** 2, 'Application input size bound')
    entries.push({ path, mode: match[1], gitBlob: match[2], bytes })
  }
  entries.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0)
  assert(entries.length > 0 && entries.length <= 20000 && entries.reduce((total, entry) => total + entry.bytes, 0) <= MAX_INPUT_BYTES)
  for (const path of ['frontend/index.html', 'frontend/package.json', 'frontend/pnpm-lock.yaml', 'frontend/vite.config.ts', 'frontend/tsconfig.json']) {
    assert(entries.some(entry => entry.path === path), 'Required application input absent: ' + path)
  }
  const packages = []
  for (const name of ['vite', '@vitejs/plugin-react', '@tailwindcss/vite', 'typescript', 'react', 'react-dom']) {
    const bytes = await readFile(resolve(dependencies, name, 'package.json'))
    assert(bytes.length <= 65536)
    const metadata = JSON.parse(bytes)
    assert.equal(metadata.name, name); assert(typeof metadata.version === 'string')
    packages.push({ name, version: metadata.version, packageJsonSha256: sha256(bytes) })
  }
  const assemblyInputs = []
  for (const name of ['application-release.mjs', 'server.mjs', 'pinned-source.mjs', 'inputs.mjs', 'private-file.mjs', 'community-clock-input.mjs']) {
    assemblyInputs.push({ name, sha256: sha256(await readFile(new URL(name, import.meta.url))) })
  }
  const identity = { version: 1, kind: 'APPLICATION_BUILD_INPUT_IDENTITY', applicationVersion: ROOT_APPLICATION_VERSION, sourceRevision: candidate, mode,
    capturePolicy: 'Tracked production source excluding test modules; all tracked public assets; entry, dependency and compiler inputs; exact optional compiled Community clock bytes. Other runtime event/actor configuration excluded.',
    inputs: await hashBlobs(repo, entries), assemblyInputs,
    compiledInputs: communityClockInput === null ? [] : [compiledClockIdentity(communityClockInput)],
    toolchain: { node: process.versions.node, nodeExecutableSha256: await fileHash(process.execPath), packages },
    emittedOutputs: { status: 'UNAVAILABLE', reason: 'NOT_BUILT_BY_THIS_PUBLISHER', files: [] } }
  const buildId = sha256(Buffer.from(canonicalJson(identity)))
  const release = Object.freeze({ version: 1, applicationVersion: identity.applicationVersion, buildId, sourceRevision: candidate })
  const manifest = { ...identity, buildId }
  assert(Buffer.byteLength(canonicalJson(manifest)) <= 4 * 1024 ** 2, 'Manifest size bound')
  return { release, manifest }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [repo, candidate, dependencies, mode, output] = process.argv.slice(2)
  assert([repo, candidate, dependencies, mode, output].every(Boolean), 'Usage: application-release.mjs REPO CANDIDATE EXISTING_NODE_MODULES vite-serve|vite-build NEW_MANIFEST.json')
  const result = await buildApplicationRelease({ repo, candidate, dependencies, mode })
  const bytes = canonicalJson(result.manifest)
  assert(Buffer.byteLength(bytes) <= 4 * 1024 ** 2)
  await writeFile(output, bytes, { flag: 'wx', mode: 0o600 })
  process.stdout.write(JSON.stringify({ ...result.release, kind: 'BUILD_INPUT_IDENTITY_ONLY', bytes: Buffer.byteLength(bytes) }) + '\n')
}
