import type { SessionPort } from '../../shared/api/http';
import { freeze } from './contract';
export interface ArchiveContext {
    readonly eventId: string | null;
    readonly generation: number;
}
export interface ArchiveContextPort {
    read(): ArchiveContext;
    subscribe(listener: () => void): () => void;
}
/** Route event is explicit. Optional canonical-session observation exports no identity or token. */
export function createArchiveContext(eventId: string | null, session?: Pick<SessionPort, 'read'>) {
    let value = freeze<ArchiveContext>({ eventId, generation: 0 }), disposed = false, signal = session?.read().signal;
    const listeners = new Set<() => void>();
    function advance(next = value.eventId) {
        if (disposed)
            return;
        value = freeze({ eventId: next, generation: value.generation + 1 });
        for (const listener of Array.from(listeners)) {
            try {
                listener();
            }
            catch {
            }
        }
    }
    function changed() {
        signal?.removeEventListener('abort', changed);
        signal = session?.read().signal;
        signal?.addEventListener('abort', changed, { once: true });
        advance();
    }
    signal?.addEventListener('abort', changed, { once: true });
    return { read: () => value, subscribe(listener: () => void) {
            listeners.add(listener);
            return () => {
                listeners.delete(listener);
            };
        },
        setEventId(next: string | null) {
            if (next !== value.eventId)
                advance(next);
        },
        dispose() {
            if (disposed)
                return;
            advance(null);
            disposed = true;
            signal?.removeEventListener('abort', changed);
            listeners.clear();
        } };
}
