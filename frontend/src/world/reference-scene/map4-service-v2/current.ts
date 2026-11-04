import layout from './layout-source.json'
import { createCompositionModel } from './model'
import { compositionRegistration } from './registration'

// Immutable Map-only candidate; original release/World authority remains unchanged.
const model = createCompositionModel(layout, compositionRegistration)
export function currentComposition() { return model }
