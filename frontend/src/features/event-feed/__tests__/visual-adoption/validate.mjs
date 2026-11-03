import { spawnSync } from 'node:child_process'
import { writeFile, mkdir } from 'node:fs/promises'
import { resolve } from 'node:path'
const root = resolve(import.meta.dirname, '../../../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
await mkdir(evidence, { recursive: true })
const checks = [
  ['tests', 'frontend/node_modules/.bin/vitest', ['run', '--root', 'frontend', '--no-cache', 'src/app/__tests__/community.test.tsx', 'src/app/__tests__/shell.test.tsx', 'src/app/community-entry', 'src/shared/ui/concert-window', 'src/features/event-feed', 'src/features/feed-thread', 'src/features/gift-discovery', 'src/features/my-gifts', 'src/features/party-discovery', 'src/features/party-authoring', 'src/features/giveaway-authoring', 'src/features/gift-pickup']],
  ['types', process.execPath, ['frontend/node_modules/typescript/bin/tsc', '--noEmit', '--incremental', 'false', '--project', 'frontend/tsconfig.json']],
  ['lint', 'frontend/node_modules/.bin/oxlint', ['frontend/src', 'frontend/vite.config.ts']],
  ['plan', 'python3', ['scripts/mcr/check-plan.py']],
  ['ui-origin', process.execPath, ['scripts/mcr/check-ui-origin.mjs']],
  ['whitespace', 'git', ['diff', '--check']],
]
const report = []
for (const [name, command, args] of checks) {
  const r = spawnSync(command, args, { cwd: root, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 })
  const log = `${r.stdout ?? ''}${r.stderr ?? ''}${r.error ? String(r.error) : ''}`.replaceAll(root, '<WORKTREE>').replaceAll('/Users/lhs/Desktop/mcr/frontend/node_modules', '<DEPENDENCIES>').replaceAll('/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64', '<NODE_RUNTIME>')
  await writeFile(resolve(evidence, `${name}.log`), log.trimEnd() ? log.trimEnd() + '\n' : '')
  report.push({ name, command: [command === process.execPath ? 'node' : command, ...args].join(' '), exitCode: r.status, signal: r.signal })
  console.log(`${name}: ${r.status}`)
}
await writeFile(resolve(evidence, 'validation.json'), JSON.stringify({ node: process.version, checks: report }, null, 2) + '\n')
if (report.some(r => r.exitCode !== 0)) process.exitCode = 1
