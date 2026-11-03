import { defineConfig } from 'vitest/config'
export default defineConfig({ cacheDir: 'src/features/auth/__tests__/reference-target/fidelity/.test-cache', test: { maxWorkers: 2, include: ['src/features/auth/__tests__/{sign-in,session}.test.{ts,tsx}', 'src/features/auth/__tests__/reference-target/*.test.tsx', 'src/app/__tests__/{shell,routes,public-entry,enrollment,recovery}.test.{ts,tsx}'] } })
