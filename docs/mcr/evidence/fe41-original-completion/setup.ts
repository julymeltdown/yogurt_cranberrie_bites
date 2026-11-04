import { webcrypto } from 'node:crypto'
// Real SHA-256 under JSDOM, retaining native Node24 crypto semantics.
Object.defineProperty(globalThis, 'crypto', { configurable: true, value: webcrypto })
