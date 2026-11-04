import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { dirname, isAbsolute, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
// This is only a job list. The accepted outer guardian creates/owns link/cache/TMP and all cleanup.
const node = process.env.MCR_FEED_NODE, cache = process.env.MCR_FEED_CACHE, cutoff = Date.parse(process.env.MCR_FEED_ACTIVE_UNTIL ?? '')
if (!node || !cache || !isAbsolute(node) || !isAbsolute(cache) || !process.env.MCR_FEED_ALLOCATION
  || process.version !== 'v24.22.2' || !(cutoff > Date.now())
  || createHash('sha256').update(readFileSync(node)).digest('hex') !== 'b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a') throw Error('Use a fresh accepted source allocation')
const frontend = resolve(dirname(fileURLToPath(import.meta.url)), '../../../..'), prefix = 'e2e/real/social/default-app/'
const jobs = [
  ['new-unit', ['--test', prefix + 'configuration.test.mjs', prefix + 'audit.test.mjs', prefix + 'source.test.mjs']],
  ['new-types', ['node_modules/typescript/bin/tsc', '-p', prefix + 'tsconfig.json', '--noEmit', '--incremental', 'false', '--pretty', 'false']],
  ['new-lint', ['node_modules/oxlint/bin/oxlint', '--deny-warnings', prefix]],
  ...['source.mjs', 'configuration.mjs', 'enrollment.mjs', 'serve.mjs', 'audit.mjs', 'check.mjs'].map(path => ['syntax-' + path, ['--check', prefix + path]]),
]
const from = process.env.MCR_DEFAULT_APP_CHECK_FROM
if (from !== undefined && from !== 'new-types') throw Error('Unsupported source continuation')
// Only a relevant failed type check may resume after the already-passed, unchanged pure cases.
for (const [name, args] of from === 'new-types' ? jobs.slice(1) : jobs) {
  if (Date.now() >= cutoff) throw Error('Source clock ended')
  const result = spawnSync(node, args, { cwd: frontend, env: { ...process.env, NODE_DISABLE_COMPILE_CACHE: '1' }, stdio: 'inherit' })
  process.stdout.write(JSON.stringify({ name, exit: result.status, signal: result.signal, actualIN03: 'NOT_RUN' }) + '\n')
  if (result.status !== 0) process.exit(result.status ?? 1)
}
