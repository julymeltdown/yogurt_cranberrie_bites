// Exact source preservation is stronger here than comparing selected handler AST nodes.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

assert.equal(process.env.PI12_SOURCE_ACCEPTED, '1')
const repo = resolve(import.meta.dirname, '../../../../..')
const readset = JSON.parse(readFileSync(resolve(repo, 'docs/mcr/evidence/parade-ink-operations/readset.json')))
const sha = bytes => createHash('sha256').update(bytes).digest('hex')
for (const entry of readset.ownedOriginals) {
  const original = execFileSync('git', ['show', `${readset.base}:${entry.path}`], { cwd: repo }).toString()
  assert.equal(sha(original), entry.sha256)
  let current = readFileSync(resolve(repo, entry.path), 'utf8')
  if (entry.path.endsWith('/index.ts')) {
    const name = entry.path.includes('/members/') ? 'Member' : 'Content'
    current = current.replace(`export { ${name}ReviewScreen } from './ui/parade-ink/${name}ReviewScreen';`,
      `export { ModerationReview as ${name}ReviewScreen } from '../reports/ui/ModerationReview';`)
  } else {
    current = current.replace(/^import \{ OperationsSurface(?:, OperationsFrame)? \} from '[^']+'\n/, '')
      .replace(/<OperationsSurface view=(?:\{summaryOnly \? 'control' : 'event'\}|"(?:places|audit|wall|closure)")>/, '')
      .replace('</OperationsSurface>', '')
    if (/\/(?:event\/ui\/EventOperations|places\/PlacesOperations|audit\/AuditOperations)\.tsx$/.test(entry.path)) current = current.replace('OperationFeedback, OperationsGate', 'OperationFeedback, OperationsFrame, OperationsGate')
  }
  assert.equal(current, original, `Protected behavior, copy and state changed in ${entry.path}`)
}
for (const entry of readset.protected) {
  assert.equal(sha(readFileSync(resolve(repo, entry.path))), entry.sha256, `Protected source changed: ${entry.path}`)
}
const changed = execFileSync('git', ['diff', '--name-only', readset.base], { cwd: repo }).toString().trim().split('\n').filter(Boolean)
const existing = new Set(readset.ownedOriginals.map(entry => entry.path))
const prefixes = [
  'frontend/src/features/admin/parade-ink-operations/', 'frontend/src/features/admin/members/ui/parade-ink/',
  'frontend/src/features/admin/content/ui/parade-ink/', 'frontend/src/app/__tests__/parade-ink-operations/',
  'frontend/e2e/visual/parade-ink-operations/', 'docs/mcr/evidence/parade-ink-operations/',
]
for (const path of changed) assert(existing.has(path) || prefixes.some(prefix => path.startsWith(prefix)) || path === 'docs/mcr/frontend/parade-ink-operations.md', `Out-of-scope path: ${path}`)
console.log(`PASS: seven exact presentation seams, ${readset.protected.length} protected source hashes, scoped changed paths`)
