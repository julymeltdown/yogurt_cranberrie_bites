export const base: string
export const root: string
export const operationIds: readonly string[]
export function readJSON(path: string): any
export function assertOperations(metadata: (id: string) => unknown, operations?: unknown, openapi?: unknown): void
export function inputManifest(): { base: string; status: { C07: string; IN03: string }; inputs: { path: string; sha256: string }[] }
