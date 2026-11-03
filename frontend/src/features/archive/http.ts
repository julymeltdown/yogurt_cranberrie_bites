import { domainOperation, matchesDomainRequest } from '../../shared/api/domain';
import { uuid } from './contract';
export type ArchiveFailure = 'unavailable' | 'offline' | 'rate-limited' | 'invalid' | 'failed' | 'changed';
export class ArchiveReadError extends Error {
    constructor(readonly failure: ArchiveFailure) {
        super('Archive read unavailable');
    }
}
export interface ArchiveReadRequest {
    readonly eventId: string;
    readonly cursor?: string;
    readonly signal: AbortSignal;
}
export interface ArchiveReader {
    read(request: ArchiveReadRequest): Promise<unknown>;
}
export const unavailableArchiveReader: ArchiveReader = { read: async () => {
        throw new ArchiveReadError('unavailable');
    } };
/** Only the registered public archive GET. No session, cookies, auth headers, retry or other endpoint. */
export function createPublicArchiveReader(fetcher: typeof fetch = globalThis.fetch): ArchiveReader {
    return { async read({ eventId, cursor, signal }) {
            const op = domainOperation('getConcertArchive')!, path = op.path.replace('{eventId}', eventId), query = { limit: 20, ...(cursor !== undefined ? { cursor } : {}) };
            if (!uuid.test(eventId) || !matchesDomainRequest(op, 'GET', path, query))
                throw new ArchiveReadError('invalid');
            const origin = globalThis.location?.origin;
            if (!origin || !/^https?:\/\//.test(origin))
                throw new ArchiveReadError('invalid');
            const url = new URL(path, origin);
            for (const [key, value] of Object.entries(query))
                url.searchParams.set(key, String(value));
            const lifetime = AbortSignal.any([signal, AbortSignal.timeout(15000)]);
            try {
                const response = await fetcher(new Request(url, { method: 'GET', credentials: 'omit', cache: 'no-store', redirect: 'error', referrerPolicy: 'no-referrer', headers: { Accept: 'application/json' }, signal: lifetime }));
                if (response.status !== 200) {
                    await response.body?.cancel();
                    throw new ArchiveReadError(response.status === 404 || response.status === 410 || response.status === 401 || response.status === 403 ? 'unavailable' : response.status === 429 ? 'rate-limited' : 'failed');
                }
                if (response.redirected || !/^application\/json(?:\s*;|$)/i.test(response.headers.get('Content-Type') ?? '') || !response.body) {
                    await response.body?.cancel();
                    throw new ArchiveReadError('invalid');
                }
                const reader = response.body.getReader(), chunks: Uint8Array[] = [];
                let size = 0;
                try {
                    while (true) {
                        lifetime.throwIfAborted();
                        const { done, value } = await reader.read();
                        if (done)
                            break;
                        size += value.byteLength;
                        if (size > 2 * 1024 * 1024)
                            throw new ArchiveReadError('invalid');
                        chunks.push(value);
                    }
                }
                finally {
                    await reader.cancel();
                    reader.releaseLock();
                }
                lifetime.throwIfAborted();
                const bytes = new Uint8Array(size);
                let offset = 0;
                for (const chunk of chunks) {
                    bytes.set(chunk, offset);
                    offset += chunk.length;
                }
                try {
                    return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)) as unknown;
                }
                catch {
                    throw new ArchiveReadError('invalid');
                }
            }
            catch (error) {
                if (error instanceof ArchiveReadError)
                    throw error;
                throw new ArchiveReadError('offline');
            }
        } };
}
