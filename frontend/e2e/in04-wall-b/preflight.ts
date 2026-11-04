import { loadConfig, NotRun } from '../real/mural-helpers.ts'
import { requireInputs } from './helpers.ts'
import { loadRuntimeAdapter } from './runtime.ts'
try {
  requireInputs(await loadConfig())
  await loadRuntimeAdapter()
  // This check starts no services, browser or network request; a present port
  // still requires actual inspect()/role/isolation verification in the journey.
  process.stdout.write(JSON.stringify({ preflight: 'INPUTS_PRESENT', realIN04: 'NOT_RUN' }) + '\n')
} catch (error) {
  process.stdout.write(JSON.stringify({ realIN04: 'NOT_RUN', code: error instanceof NotRun ? error.code : 'PREFLIGHT_FAILURE' }) + '\n')
  process.exitCode = error instanceof NotRun ? 2 : 1
}
