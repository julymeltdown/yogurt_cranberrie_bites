import { lstat, readFile } from 'node:fs/promises'
import { isAbsolute } from 'node:path'
import { hash, need } from '../real/mural-helpers.ts'
import type { GateConfig, RuntimeQualificationPort } from '../real/mural-helpers.ts'
import { validateMuralProductionConfiguration } from '../../src/app/world-entry/mural-production/configuration.ts'
import type { MuralProductionConfiguration } from '../../src/app/world-entry/mural-production/configuration.ts'

/** Local implementation seam, NOT a new HTTP schema or authority endpoint.
 * The actual responsible producer supplies a self-contained .mjs implementation
 * and its qualified SHA. A JSON/config statement cannot implement this port.
 */
export interface RuntimeAdapter extends RuntimeQualificationPort {
  muralBinding(config: GateConfig): Promise<{
    candidate: string; eventId: string; expiresAt: string
    configuration: unknown; phaserModuleUrl: string
  }>
}
export interface QualifiedBinding {
  configuration: MuralProductionConfiguration
  phaserModuleUrl: string
  expiresAt: string
}
/** The producer resolves this exact transformed import to its allocated Vite
 * cache. Retain the version query: a different URL can create another CanvasPool.
 */
export function qualifiedPhaserModuleUrl(value: unknown, origin: string): string {
  need(typeof value === 'string', 'SAME_APP_DEV_PHASER_MODULE_REQUIRED')
  const url = new URL(value, origin)
  need(url.origin === origin && !url.username && !url.password && !value.includes('#')
    && /^\/@fs\/\/?(?:[^/]+\/)+deps\/phaser\.js$/.test(url.pathname)
    && /^\?v=[a-f0-9]+$/.test(url.search)
    && (value === url.href || value === url.pathname + url.search), 'SAME_APP_DEV_PHASER_MODULE_REQUIRED')
  // Reject encoded separators/control bytes while retaining legitimate encoded
  // directory names. Cache/file identity itself belongs to the source producer.
  need(url.pathname.split('/').every(part => !/[\\/\u0000-\u001f\u007f]/.test(decodeURIComponent(part))), 'SAME_APP_DEV_PHASER_MODULE_REQUIRED')
  return url.href
}
export async function loadRuntimeAdapter(path = process.env.MCR_IN04_B_RUNTIME_ADAPTER, expected = process.env.MCR_IN04_B_RUNTIME_ADAPTER_SHA256): Promise<RuntimeAdapter> {
  need(path && isAbsolute(path) && path.endsWith('.mjs') && expected && /^[a-f0-9]{64}$/.test(expected), 'ACTUAL_OWNER_RUNTIME_MODULE_AND_QUALIFIED_SHA_REQUIRED')
  const st = await lstat(path)
  need(st.isFile() && !st.isSymbolicLink() && st.uid === process.getuid?.() && !(st.mode & 0o022) && st.size <= 1024 * 1024, 'OWNED_IMMUTABLE_RUNTIME_MODULE_REQUIRED')
  const source = await readFile(path)
  need(hash(source) === expected, 'RUNTIME_MODULE_SOURCE_HASH_MISMATCH')
  // Import the verified bytes, preventing a later path replacement from silently
  // changing the loaded entry. Self-contained modules may import node built-ins;
  // relative imports are unsupported and never resolved against this QA checkout.
  const module = await import('data:text/javascript;base64,' + source.toString('base64')) as { runtimeQualification?: RuntimeAdapter }
  const port = module.runtimeQualification
  need(port && typeof port.inspect === 'function' && typeof port.observeRestart === 'function' && typeof port.muralBinding === 'function', 'ACTUAL_OWNER_RUNTIME_IMPLEMENTATION_REQUIRED')
  return port
}
export async function qualifiedBinding(port: RuntimeAdapter, config: GateConfig): Promise<QualifiedBinding> {
  const binding = await port.muralBinding(config)
  need(binding.candidate === config.releaseRevision && binding.eventId === config.eventId, 'CURRENT_MURAL_BINDING_SCOPE_REQUIRED')
  const expiry = Date.parse(binding.expiresAt)
  need(expiry > Date.now() && expiry <= Date.now() + 15 * 60000, 'FRESH_MURAL_BINDING_REQUIRED')
  const configuration = validateMuralProductionConfiguration(binding.configuration)
  need(configuration, 'EXACT_PRODUCTION_MURAL_CONFIGURATION_REQUIRED')
  const phaserModuleUrl = qualifiedPhaserModuleUrl(binding.phaserModuleUrl, config.origin)
  return { configuration, phaserModuleUrl, expiresAt: binding.expiresAt }
}
