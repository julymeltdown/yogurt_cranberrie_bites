// Read-only use of the existing installed dependency tree; no installs or links.
import { registerHooks } from 'node:module'
import { pathToFileURL } from 'node:url'
import { resolve } from 'node:path'
const tooling = process.env.PI09_TOOLING_ROOT
if (!tooling) throw Error('An explicitly booked existing PI09_TOOLING_ROOT is required')
const toolParent = pathToFileURL(resolve(tooling, 'package.json')).href
registerHooks({ resolve(specifier, context, nextResolve) {
  try { return nextResolve(specifier, context) }
  catch (error) {
    if (error.code !== 'ERR_MODULE_NOT_FOUND' || !/^(?:[a-z@])/.test(specifier)) throw error
    return nextResolve(specifier, { ...context, parentURL: toolParent })
  }
} })
