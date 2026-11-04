import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { spawnSync } from 'node:child_process'
import { readFile, writeFile, mkdir, readdir, mkdtemp, rm } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { stripVTControlCharacters } from 'node:util'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
if (!dependencies) throw Error('Set MCR_FRONTEND_DEPENDENCIES')
const require = createRequire(resolve(dependencies, 'package.json')), frontend = resolve(import.meta.dirname, '../../../../..'), checkout = resolve(frontend, '..')
const evidence = resolve(checkout, 'docs/mcr/evidence/avatar-motion-adoption'), temporary = await mkdtemp('/private/tmp/mcr-motion-checks-')
await mkdir(evidence, { recursive: true })
const node = process.execPath, config = 'src/world/avatar/__tests__/motion-adoption/vitest.config.mjs'
const prefixes = ['src/entities/avatar/runtime/motion-adoption', 'src/world/avatar/motion-adoption', 'src/features/avatar/motion-adoption',
  'src/world/avatar/__tests__/motion-adoption', 'src/features/avatar/__tests__/motion-adoption', 'src/app/character-entry/__tests__/motion-adoption']
const checks = [
  ['unit', node, [resolve(require.resolve('vitest/package.json'), '../vitest.mjs'), 'run', '--configLoader', 'runner', '--root', frontend, '--config', resolve(frontend, config)]],
  ['types', node, [resolve(frontend, 'src/world/avatar/__tests__/motion-adoption/typecheck.mjs')]],
  ['application-types', node, [resolve(frontend, 'src/world/avatar/__tests__/motion-adoption/typecheck.mjs'), '--application']],
  ['lint', node, [resolve(dependencies, 'node_modules/oxlint/bin/oxlint'), ...prefixes.map(p => resolve(frontend, p))]],
  ['build', node, [resolve(frontend, 'src/world/avatar/__tests__/motion-adoption/build.mjs')]],
  ['plan', 'python3', [resolve(checkout, 'scripts/mcr/check-plan.py')]],
  ['ui-origin', node, [resolve(checkout, 'scripts/mcr/check-ui-origin.mjs')]],
  ...(process.argv.includes('--native') ? [['native', node, [resolve(frontend, 'src/app/character-entry/__tests__/motion-adoption/native.mjs')]]] : []),
]
const clean = text => stripVTControlCharacters(text.split(checkout).join('<checkout>').split(dependencies).join('<retained-frontend>').split(node).join('<node24>'))
const report = { result: 'FAIL', qualification: 'TEST_ONLY consumer checkpoint; production Root/C06/service/device/multiuser NOT_RUN', checks: [], sourcePins: {} }
try {
  for (const [name, executable, args] of checks) {
    const result = spawnSync(executable, args, { cwd: name === 'unit' ? frontend : checkout, env: { ...process.env, MCR_MOTION_CACHE: resolve(temporary, 'vitest') }, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 })
    await writeFile(resolve(evidence, `${name}.log`), clean((result.stdout ?? '') + (result.stderr ?? '')).trimEnd() + '\n')
    report.checks.push({ name, command: clean([executable, ...args].join(' ')), exit: result.status, error: result.error?.message ?? null })
    console.log(JSON.stringify(report.checks.at(-1)))
    if (result.status !== 0 || result.error) throw Error(`Check ${name} failed`)
  }
  for (const prefix of prefixes) for (const name of await readdir(resolve(frontend, prefix), { recursive: true })) {
    if (!/\.(ts|tsx|mjs|html|css|json)$/.test(name)) continue
    const relative = `frontend/${prefix}/${name}`
    report.sourcePins[relative] = createHash('sha256').update(await readFile(resolve(checkout, relative))).digest('hex')
  }
  report.result = 'PASS'
} catch (error) { report.failure = String(error); process.exitCode = 1 }
finally { await writeFile(resolve(evidence, 'checks.json'), JSON.stringify(report, null, 2) + '\n'); await rm(temporary, { recursive: true, force: true }) }
