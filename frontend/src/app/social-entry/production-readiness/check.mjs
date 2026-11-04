import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { isAbsolute, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
const node = process.env.MCR_FEED_NODE, cache = process.env.MCR_FEED_CACHE, cutoff = Date.parse(process.env.MCR_FEED_ACTIVE_UNTIL ?? '')
if (!node || !cache || !isAbsolute(node) || !isAbsolute(cache) || !process.env.MCR_FEED_ALLOCATION
  || process.version !== 'v24.22.2' || !(cutoff > Date.now())
  || createHash('sha256').update(readFileSync(node)).digest('hex') !== 'b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a') throw Error('Use a fresh accepted source allocation')
const frontend = resolve(fileURLToPath(new URL('.', import.meta.url)), '../../../..'), prefix = 'src/app/social-entry/production-readiness/'
const jobs = [
  ['focused-unit', ['node_modules/vitest/vitest.mjs', 'run', '--configLoader', 'native', '--config', prefix + 'vitest.config.ts']],
  ['focused-types', ['node_modules/typescript/bin/tsc', '-p', prefix + 'tsconfig.json', '--noEmit', '--incremental', 'false', '--pretty', 'false']],
  ['focused-lint', ['node_modules/oxlint/bin/oxlint', '--deny-warnings', prefix]],
  ['launcher-syntax', ['--check', prefix + 'check.mjs']],
]
for (const [name, args] of jobs) {
  if (Date.now() >= cutoff) throw Error('Source clock ended')
  const result = spawnSync(node, args, { cwd: frontend, env: { ...process.env, NODE_DISABLE_COMPILE_CACHE: '1' }, stdio: 'inherit' })
  process.stdout.write(JSON.stringify({ name, exit: result.status, signal: result.signal, actualIN03: 'NOT_RUN' }) + '\n')
  if (result.status !== 0) process.exit(result.status ?? 1)
}
