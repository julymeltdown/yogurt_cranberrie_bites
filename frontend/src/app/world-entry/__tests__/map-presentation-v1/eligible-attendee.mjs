import original from './source-inputs/operator-bootstrap.json' with { type: 'json' }
import scenario from './source-inputs/eligible-attendee-scenario.json' with { type: 'json' }

// Explicit test scenario only. Never imported by application production entrypoints.
export function eligibleAttendeeFixture() {
  const input = structuredClone(original)
  input.capabilities.world = structuredClone(scenario.projection['capabilities.world'])
  return input
}
export const eligibleAttendeeScenario = scenario
