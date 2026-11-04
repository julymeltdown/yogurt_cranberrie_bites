import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { lstatSync, readFileSync, realpathSync } from 'node:fs'
import { isAbsolute, resolve, sep } from 'node:path'

export const frozenBridge = '37add6d996eacb7674c1c220150870d7090d5e09'
export const releaseHelpers = ['application-release.mjs', 'server.mjs', 'pinned-source.mjs', 'inputs.mjs', 'private-file.mjs']
  .map(name => 'frontend/e2e/runtime-adapters/in04-wall-b/' + name)
export const bridgePaths = ['frontend/src/main.tsx', 'frontend/src/app/ports.ts',
  'frontend/src/app/world-entry/production.ts', 'frontend/src/app/world-entry/social-availability.ts',
  'frontend/src/app/world-entry/configuration.ts', 'frontend/src/app/world-entry/resolver.ts',
  'frontend/src/app/social-entry/production-readiness/index.ts', 'frontend/src/world/composition/policy.ts',
  'frontend/src/shared/release/application.ts', ...releaseHelpers]
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex')
const fail = () => { throw Error('NOT_RUN: default application source unavailable') }
export const isRevision = value => typeof value === 'string' && /^[0-9a-f]{40}$/.test(value)
export function productionPath(path) {
  return typeof path === 'string' && !path.includes('/__tests__/') && !path.includes('/fixtures/')
    && !/\.(?:test|spec)\.[^/]+$/.test(path) && !path.includes('..')
    && (path.startsWith('frontend/src/') || path.startsWith('frontend/public/')
      || path.startsWith('contracts/') || releaseHelpers.includes(path)
      || ['frontend/index.html', 'frontend/package.json', 'frontend/pnpm-lock.yaml'].includes(path))
}
/** Read only the pinned Git object database; never check out or replace another owner's files. */
export function frozenSource(repository, revision) {
  if (!isAbsolute(repository) || !isRevision(revision)) return fail()
  const git = args => execFileSync('git', args, { cwd: repository, stdio: ['ignore', 'pipe', 'ignore'], maxBuffer: 16 * 1024 ** 2 })
  for (const path of bridgePaths) {
    if (sha256(git(['show', `${revision}:${path}`])) !== sha256(git(['show', `${frozenBridge}:${path}`]))) return fail()
  }
  const listing = git(['ls-tree', '-r', '-z', revision, '--', 'frontend/src', 'frontend/public', 'frontend/index.html',
    'frontend/package.json', 'frontend/pnpm-lock.yaml', 'contracts', ...releaseHelpers]).toString().split('\0').filter(Boolean)
  const files = listing.map(line => {
    const at = line.indexOf('\t'), [mode, type, blob] = line.slice(0, at).split(' '), path = line.slice(at + 1)
    if (!productionPath(path)) return null
    if (type !== 'blob' || !['100644', '100755'].includes(mode)) return fail()
    return { path, mode, blob, sha256: sha256(git(['show', `${revision}:${path}`])) }
  }).filter(file => file !== null)
  for (const path of bridgePaths) {
    const actual = files.find(file => file.path === path)
    if (!actual || actual.sha256 !== sha256(git(['show', `${frozenBridge}:${path}`]))) return fail()
  }
  if (!files.some(file => file.path === 'frontend/index.html')) return fail()
  return { revision, frozenBridge, files, manifestSha256: sha256(JSON.stringify(files)) }
}
/** The serving tree is borrowed read-only, not the worker's stale physical main.tsx. */
export function bindServingTree(proof, root) {
  if (!isAbsolute(root) || realpathSync(root) !== resolve(root)) return fail()
  const paths = proof.files.map(file => {
    const path = resolve(root, file.path)
    if (!path.startsWith(root + sep)) return fail()
    const info = lstatSync(path)
    if (!info.isFile() || info.isSymbolicLink() || realpathSync(path) !== path || sha256(readFileSync(path)) !== file.sha256) return fail()
    return { ...file, pathOnDisk: path, fingerprint: `${info.ino}:${info.size}:${info.mtimeMs}:${info.ctimeMs}` }
  })
  return { ...proof, root, paths }
}
export function servingTreeCurrent(proof) {
  try {
    return proof.paths.every(file => {
      const value = lstatSync(file.pathOnDisk)
      return value.isFile() && !value.isSymbolicLink() && realpathSync(file.pathOnDisk) === file.pathOnDisk
        && `${value.ino}:${value.size}:${value.mtimeMs}:${value.ctimeMs}` === file.fingerprint
    })
  } catch { return false }
}
