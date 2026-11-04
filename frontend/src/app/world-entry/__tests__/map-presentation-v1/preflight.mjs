// Committed synthetic source modes only. Never reads/hashes an arbitrary authenticated C03 body.
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { inspectInputReadiness } from './input-readiness.mjs'
import { eligibleAttendeeFixture, eligibleAttendeeScenario } from './eligible-attendee.mjs'

const args = process.argv.slice(2)
if (args.length > 1 || (args.length === 1 && args[0] !== '--eligible-attendee-contract-fixture')) {
  process.stdout.write(JSON.stringify({ status: 'INPUT_NOT_READY', productionAdmitted: false, reasons: ['UNSUPPORTED_SOURCE_MODE'] }) + '\n')
  process.exitCode = 2
} else try {
  const selectedFixture = process.argv[2] === '--eligible-attendee-contract-fixture'
  const path = new URL('./source-inputs/operator-bootstrap.json', import.meta.url)
  const bytes = await readFile(path)
  const input = selectedFixture ? eligibleAttendeeFixture() : JSON.parse(bytes.toString('utf8'))
  const result = inspectInputReadiness(input)
  process.stdout.write(JSON.stringify({ ...result, scenario: selectedFixture ? eligibleAttendeeScenario : 'raw-public-input',
    committedOperatorSourceSha256: 'sha256:' + createHash('sha256').update(bytes).digest('hex'),
    selectedSyntheticSourceSha256: 'sha256:' + createHash('sha256').update(JSON.stringify(input)).digest('hex') }) + '\n')
  // A zero exit is only a candidate for producer qualification, never authority to start a native job.
  process.exitCode = result.status === 'INPUT_NOT_READY' ? 2 : 0
} catch {
  process.stdout.write(JSON.stringify({ status: 'INPUT_NOT_READY', productionAdmitted: false, reasons: ['UNREADABLE_OR_INVALID_JSON'] }) + '\n')
  process.exitCode = 2
}
