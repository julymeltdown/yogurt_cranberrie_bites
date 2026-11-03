import { freeze, projectArchive, sameMural, uuid, ordered, type Archive } from './contract';
import type { ArchiveContextPort, ArchiveContext } from './context';
import { ArchiveReadError, type ArchiveFailure, type ArchiveReader } from './http';
export interface ArchiveMediaRequest {
    readonly eventId: string;
    readonly archiveRevision: number;
    readonly postId: string;
    readonly mediaId: string;
    readonly signal: AbortSignal;
}
/** Public archive association must be checked by the producer, never a protected live-media fallback. */
export interface ArchiveMediaPort {
    read(request: ArchiveMediaRequest): Promise<{
        eventId: string;
        archiveRevision: number;
        postId: string;
        mediaId: string;
        blob: Blob;
    }>;
}
export interface ArchiveState {
    readonly eventId: string | null;
    readonly phase: 'idle' | 'loading' | 'ready' | 'error';
    readonly data: Archive | null;
    readonly failure: ArchiveFailure | null;
    readonly media: {
        readonly postId: string;
        readonly mediaId: string;
        readonly url: string;
    } | null;
    readonly mediaPhase: 'idle' | 'loading' | 'unavailable';
    readonly page: number;
}
export function createArchiveController({ context, reader, media }: {
    context: ArchiveContextPort;
    reader: ArchiveReader;
    media?: ArchiveMediaPort;
}) {
    let observed: ArchiveContext = { eventId: null, generation: -1 }, state: ArchiveState = freeze({ eventId: null, phase: 'idle', data: null, failure: null, media: null, mediaPhase: 'idle', page: 0 });
    let disposed = false, serial = 0, flight: AbortController | null = null, imageSerial = 0, imageFlight: AbortController | null = null, baseline: {
        revision: number;
        final_mural: Archive['final_mural'];
        last: {
            created_at: string;
            post_id: string;
        } | null;
    } | null = null;
    let invalidated = false;
    const removedPosts = new Set<string>();
    const cursors = new Set<string>(), postIds = new Set<string>(), listeners = new Set<() => void>();
    const publish = (patch: Partial<ArchiveState>) => {
        if (disposed)
            return;
        state = freeze({ ...state, ...patch });
        for (const fn of Array.from(listeners)) {
            try {
                fn();
            }
            catch {
            }
        }
    };
    function clearImage() {
        imageSerial++;
        imageFlight?.abort();
        imageFlight = null;
        if (state.media)
            URL.revokeObjectURL(state.media.url);
    }
    function clear() {
        serial++;
        flight?.abort();
        flight = null;
        clearImage();
        baseline = null;
        cursors.clear();
        postIds.clear();
        publish({ phase: 'idle', data: null, failure: null, media: null, mediaPhase: 'idle', page: 0 });
    }
    function sync() {
        const next = context.read();
        if (next.eventId !== observed.eventId || next.generation !== observed.generation) {
            observed = freeze({ eventId: next.eventId, generation: next.generation });
            removedPosts.clear();
            invalidated = false;
            clear();
            publish({ eventId: next.eventId });
        }
    }
    const stop = context.subscribe(sync);
    sync();
    function current(c: ArchiveContext) {
        const n = context.read();
        return !disposed && n.eventId === c.eventId && n.generation === c.generation;
    }
    async function load(more = false) {
        sync();
        if (disposed || state.phase === 'loading')
            return;
        const captured = observed, eventId = captured.eventId, cursor = more ? state.data?.feed.next_cursor : null;
        if (!eventId || !uuid.test(eventId)) {
            clear();
            publish({ phase: 'error', failure: 'unavailable' });
            return;
        }
        if (more && !cursor)
            return;
        if (!more) {
            baseline = null;
            cursors.clear();
            postIds.clear();
        }
        const page = more ? state.page + 1 : 1;
        const id = ++serial;
        flight?.abort();
        const controller = new AbortController();
        flight = controller;
        clearImage();
        publish({ phase: 'loading', data: null, failure: null, media: null, mediaPhase: 'idle' });
        try {
            if (page > 50 || invalidated)
                throw new ArchiveReadError('changed');
            const input = await reader.read({ eventId, ...(cursor ? { cursor } : {}), signal: controller.signal });
            if (!current(captured) || id !== serial || controller.signal.aborted)
                return;
            const data = projectArchive(input, eventId);
            if (data.feed.items.some(p => removedPosts.has(p.post_id)))
                throw new ArchiveReadError('changed');
            if (more && baseline && (data.revision !== baseline.revision || !sameMural(data.final_mural, baseline.final_mural)))
                throw new ArchiveReadError('changed');
            if (data.feed.next_cursor && (data.feed.next_cursor === cursor || cursors.has(data.feed.next_cursor)) || data.feed.items.some(p => postIds.has(p.post_id)))
                throw new ArchiveReadError('invalid');
            if (more && baseline?.last && data.feed.items.length && !ordered([baseline.last, data.feed.items[0]!], p => p.post_id))
                throw new ArchiveReadError('invalid');
            if (cursor)
                cursors.add(cursor);
            for (const p of data.feed.items)
                postIds.add(p.post_id);
            const last = data.feed.items.at(-1);
            baseline = { revision: data.revision, final_mural: data.final_mural, last: last ? { created_at: last.created_at, post_id: last.post_id } : baseline?.last ?? null };
            publish({ phase: 'ready', data, failure: null, page });
        }
        catch (error) {
            if (current(captured) && id === serial && !controller.signal.aborted) {
                baseline = null;
                publish({ phase: 'error', data: null, media: null, mediaPhase: 'idle', failure: error instanceof ArchiveReadError ? error.failure : 'invalid' });
            }
        }
    }
    async function openMedia(postId: string, mediaId: string) {
        sync();
        const captured = observed, data = state.data;
        if (disposed || state.phase !== 'ready' || !data?.feed.items.some(p => p.post_id === postId && p.media_ids.includes(mediaId)))
            return;
        clearImage();
        publish({ media: null, mediaPhase: media ? 'loading' : 'unavailable' });
        if (!media)
            return;
        const id = ++imageSerial, controller = new AbortController();
        imageFlight = controller;
        try {
            const result = await media.read({ eventId: data.event_id, archiveRevision: data.revision, postId, mediaId, signal: controller.signal });
            if (!current(captured) || id !== imageSerial || controller.signal.aborted || state.data !== data)
                return;
            if (result.eventId !== data.event_id || result.archiveRevision !== data.revision || result.postId !== postId || result.mediaId !== mediaId || result.blob.size < 1 || result.blob.size > 10 * 1024 * 1024 || !['image/png', 'image/jpeg', 'image/webp'].includes(result.blob.type))
                throw new Error('Invalid public media');
            publish({ media: { postId, mediaId, url: URL.createObjectURL(result.blob) }, mediaPhase: 'idle' });
        }
        catch {
            if (current(captured) && id === imageSerial && !controller.signal.aborted)
                publish({ media: null, mediaPhase: 'unavailable' });
        }
    }
    return { read: () => state, subscribe(fn: () => void) {
            listeners.add(fn);
            return () => {
                listeners.delete(fn);
            };
        }, load, openMedia,
        invalidatePost(eventId:string,postId:string) {
            if(disposed||context.read().eventId!==eventId||!uuid.test(postId))return;
            removedPosts.add(postId);
            if(removedPosts.size>1000){invalidated=true;removedPosts.clear();}
            if(state.phase==='loading'||state.data?.feed.items.some(p=>p.post_id===postId)){
                clear();publish({phase:'error',failure:'changed'});
            }
        },
        dispose() {
            if (disposed)
                return;
            clear();
            removedPosts.clear();
            stop();
            disposed = true;
            listeners.clear();
        } };
}
export type ArchiveController = ReturnType<typeof createArchiveController>;
