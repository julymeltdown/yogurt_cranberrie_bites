import { expect, it } from 'vitest'
import probes from '../../../../../contracts/fixtures/asset-semantics/illustrated-cases.json'
import theme from '../../../../../contracts/fixtures/assets/illustrated/valid-theme.json'
import { schemaValid } from '../../map/schema'
import { validateScenery, type Scenery } from '../../illustrated/model'
// Same independently authored 25 metadata decisions consumed by Python and Rust validators.
it.each(probes.cases)('matches shared C06 semantics: $name', probe => {
  let accepted = false
  if (schemaValid({ ...theme, scenery: probe.scenery }, 'theme')) {
    try { validateScenery(probe.scenery as unknown as Scenery, 160, 160); accepted = true } catch { /* Invalid probe. */ }
  }
  expect(accepted).toBe(probe.valid)
})
