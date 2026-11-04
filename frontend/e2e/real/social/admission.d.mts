export function admitHTTP(config: unknown): { status: string; claim: string; missing: { input: string; owner: string }[]; IN03: string; unavailable: { input: string; owner: string }[] }
export function readRunConfig(path?: string): any
export function admitOriginal(config: unknown): ReturnType<typeof admitHTTP>
