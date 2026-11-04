import { defineConfig } from '@playwright/test'
import { isAbsolute } from 'node:path'
import { lstatSync, realpathSync } from 'node:fs'
const output = process.env.MCR_DEFAULT_APP_ARTIFACTS
if (!output || !isAbsolute(output)) throw Error('Use an allocated absolute private artifact directory')
const info = lstatSync(output), uid = process.getuid?.()
if (uid === undefined || !info.isDirectory() || info.isSymbolicLink() || info.uid !== uid || (info.mode & 0o777) !== 0o700 || realpathSync(output) !== output) throw Error('Private artifacts required')
export default defineConfig({ testDir: '.', testMatch: 'default-app.spec.ts', outputDir: output,
  workers: 1, fullyParallel: false, retries: 0, timeout: 180_000, globalTimeout: 200_000,
  reporter: [['line']], use: { trace: 'off', video: 'off', screenshot: 'off' },
  // No webServer: real serving is separately allocated and explicitly bound to its private packet.
})
