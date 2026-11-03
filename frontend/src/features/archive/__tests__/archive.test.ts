// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createArchiveContext } from '../context';
import { createArchiveController, type ArchiveMediaPort } from '../controller';
import { createPublicArchiveReader, ArchiveReadError, type ArchiveReadRequest } from '../http';
import { createMemorySession } from '../../../shared/api/http';
import { projectArchive } from '../contract';
import { page, id, eventId, deferred, imageBlob } from './fixtures/data';
const cleanups: (() => void)[] = [];
afterEach(() => {
    cleanups.splice(0).forEach(fn => fn());
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
});
function setup(media?: ArchiveMediaPort) {
    const context = createArchiveContext(eventId), calls: ArchiveReadRequest[] = [];
    let read = (r: ArchiveReadRequest): Promise<unknown> => Promise.resolve(page(!!r.cursor, r.eventId));
    const controller = createArchiveController({ context, reader: { read(r) {
                calls.push(r);
                return read(r);
            } }, ...(media ? { media } : {}) });
    cleanups.push(() => {
        controller.dispose();
        context.dispose();
    });
    return { context, controller, calls, setRead(next: typeof read) {
            read = next;
        } };
}
describe('strict public archive projection', () => {
    it('keeps only permitted archive posts and final pixels, no live/user cooldown state', () => {
        const value = projectArchive(page(), eventId);
        expect(value.feed.items).toHaveLength(1);
        expect(value.final_mural.pixels).toHaveLength(16384);
        expect(Object.keys(value.final_mural)).not.toContain('cooldown_ms');
        expect(Object.isFrozen(value.final_mural.pixels)).toBe(true);
    });
    it.each(['handles', 'physical_beacon', 'recipients', 'private_activity', 'author', 'state'])('rejects %s in an archive post', async (key) => {
        const h = setup(), value = page();
        Object.assign(value.feed.items[0]!, { [key]: 'PRIVATE_SENTINEL' });
        h.setRead(async () => value);
        await h.controller.load();
        expect(h.controller.read()).toMatchObject({ phase: 'error', data: null, failure: 'invalid' });
        expect(JSON.stringify(h.controller.read())).not.toContain('PRIVATE_SENTINEL');
    });
    it.each(['root', 'mural', 'feed'])('rejects private extras in %s', async (location) => {
        const h = setup(), value = page();
        Object.assign(location === 'root' ? value : location === 'mural' ? value.final_mural : value.feed, { private_handle: 'PRIVATE_SENTINEL' });
        h.setRead(async () => value);
        await h.controller.load();
        expect(h.controller.read().data).toBeNull();
    });
    it('rejects foreign, mutable, malformed and deleted resources', () => {
        for (const mutate of [(v: ReturnType<typeof page>) => {
                v.event_id = id(99);
            }, v => {
                v.final_mural.event_id = id(99);
            }, v => {
                v.final_mural.state = 'EDITABLE';
            }, v => {
                v.final_mural.pixels.pop();
            }, v => {
                v.final_mural.palette[0] = 'javascript:x';
            }, v => {
                Object.assign(v.feed.items[0]!, { state: 'DELETED' });
            }, v => {
                v.feed.items[0]!.media_ids.push(id(30));
            }, v => {
                v.feed.items[0]!.created_at = '2026-02-30T00:00:00Z';
            }] as ((v: ReturnType<typeof page>) => void)[]) {
            const value = page();
            mutate(value);
            expect(() => projectArchive(value, eventId)).toThrow();
        }
    });
});
describe('bounded pages, context fencing and deletion', () => {
    it('replaces pages, preserves the frozen snapshot and does not accumulate old post text', async () => {
        const h = setup();
        await h.controller.load();
        await h.controller.load(true);
        expect(h.calls.map(r => r.cursor)).toEqual([undefined, 'page-two']);
        expect(h.controller.read()).toMatchObject({ phase: 'ready', page: 2, data: { revision: 7, feed: { next_cursor: null } } });
        expect(JSON.stringify(h.controller.read())).not.toContain('We sang every word');
        await h.controller.load(true);
        expect(h.calls).toHaveLength(2);
    });
    it('shows loading without prior records and supports empty public pages', async () => {
        const h = setup();
        await h.controller.load();
        const held = deferred<unknown>();
        h.setRead(() => held.promise);
        const pending = h.controller.load();
        expect(h.controller.read()).toMatchObject({ phase: 'loading', data: null });
        const empty = page();
        empty.feed.items = [];
        empty.feed.next_cursor = null as never;
        held.resolve(empty);
        await pending;
        expect(h.controller.read()).toMatchObject({ phase: 'ready', data: { feed: { items: [] } } });
    });
    it.each(['revision', 'pixels', 'cursor', 'duplicate', 'order'])('discards unsafe next page on %s change', async (kind) => {
        const h = setup();
        await h.controller.load();
        const value = page(true);
        if (kind === 'revision')
            value.revision++;
        if (kind === 'pixels')
            value.final_mural.pixels[0] = 9;
        if (kind === 'cursor')
            value.feed.next_cursor = 'page-two';
        if (kind === 'duplicate')
            value.feed.items[0]!.post_id = id(12);
        if (kind === 'order')
            value.feed.items[0]!.created_at = '2026-11-07T15:00:00Z';
        h.setRead(async () => value);
        await h.controller.load(true);
        expect(h.controller.read()).toMatchObject({ phase: 'error', data: null });
    });
    it('drops a late old-event response and exposes only the new event', async () => {
        const h = setup(), held = deferred<unknown>();
        h.setRead(() => held.promise);
        const pending = h.controller.load();
        h.context.setEventId(id(2));
        expect(h.calls[0]!.signal.aborted).toBe(true);
        expect(h.controller.read().data).toBeNull();
        held.resolve(page());
        await pending;
        expect(h.controller.read().phase).toBe('idle');
        h.setRead(async (r) => page(false, r.eventId));
        await h.controller.load();
        expect(h.controller.read().data?.event_id).toBe(id(2));
    });
    it.each(['login', 'logout', 'account', 'event'] as const)('clears public snapshots on canonical %s change without retaining identity', async (kind) => {
        const session = createMemorySession();
        if (kind !== 'login')
            session.replace(0, { userId: id(50), eventId, accessToken: 'private-token' });
        const context = createArchiveContext(eventId, session), controller = createArchiveController({ context, reader: { read: async () => page() } });
        try {
            await controller.load();
            session.replace(session.read().generation, kind === 'logout' ? null : { userId: id(kind === 'account' ? 51 : 50), eventId: kind === 'event' ? id(2) : eventId, accessToken: 'another-private-token' });
            expect(controller.read().data).toBeNull();
            expect(JSON.stringify(context.read())).not.toMatch(/private-token|userId|accessToken/);
            expect(controller.read().phase).toBe('idle');
        }
        finally {
            controller.dispose();
            context.dispose();
        }
    });
    it('removes revoked/deleted posts and final canvas data synchronously', async () => {
        const h = setup();
        await h.controller.load();
        h.controller.invalidatePost(eventId,id(12));
        expect(h.controller.read()).toMatchObject({ phase: 'error', data: null, failure: 'changed' });
    });
    it('never retains a previous page after unavailable/offline responses', async () => {
        const h = setup();
        for (const reason of ['unavailable', 'offline'] as const) {
            h.setRead(async () => page());
            await h.controller.load();
            h.setRead(async () => {
                throw new ArchiveReadError(reason);
            });
            await h.controller.load();
            expect(h.controller.read()).toMatchObject({ data: null, failure: reason });
        }
    });
    it('drops results after disposal', async () => {
        const h = setup(), held = deferred<unknown>();
        h.setRead(() => held.promise);
        const pending = h.controller.load();
        h.controller.dispose();
        held.resolve(page());
        await pending;
        expect(h.controller.read().data).toBeNull();
    });
});
describe('public-only HTTP boundary', () => {
    it('binds the registered GET/query with omitted credentials, no-store, no authorization or mutation', async () => {
        const requests: Request[] = [], reader = createPublicArchiveReader(async (input) => {
            requests.push(input as Request);
            return Response.json(page());
        });
        await reader.read({ eventId, signal: new AbortController().signal });
        const r = requests[0]!;
        expect(r.method).toBe('GET');
        expect(new URL(r.url).pathname).toBe('/api/concert/events/' + eventId + '/archive');
        expect(new URL(r.url).searchParams.get('limit')).toBe('20');
        expect(r.credentials).toBe('omit');
        expect(r.cache).toBe('no-store');
        expect(r.redirect).toBe('error');
        expect(r.headers.has('Authorization')).toBe(false);
        expect(r.headers.has('Cookie')).toBe(false);
        expect(r.body).toBeNull();
    });
    it.each([401, 403, 404, 410, 429, 503])('fails safely for HTTP %s without exposing diagnostics', async (status) => {
        const h = setup(), reader = createPublicArchiveReader(async () => Response.json({ private_handle: 'PRIVATE_SENTINEL' }, { status }));
        h.setRead(r => reader.read(r));
        await h.controller.load();
        expect(h.controller.read().data).toBeNull();
        expect(JSON.stringify(h.controller.read())).not.toContain('PRIVATE_SENTINEL');
    });
    it('rejects an invalid route without fetching', async () => {
        const fetcher = vi.fn(), reader = createPublicArchiveReader(fetcher);
        await expect(reader.read({ eventId: '../private', signal: new AbortController().signal })).rejects.toBeInstanceOf(ArchiveReadError);
        expect(fetcher).not.toHaveBeenCalled();
    });
    it.each(['html', 'bad json', 'oversize'])('rejects %s responses', async (kind) => {
        const reader = createPublicArchiveReader(async () => kind === 'html' ? new Response('<html>private</html>', { headers: { 'Content-Type': 'text/html' } }) : new Response(kind === 'oversize' ? 'x'.repeat(2 * 1024 * 1024 + 1) : '{bad', { headers: { 'Content-Type': 'application/json' } }));
        await expect(reader.read({ eventId, signal: new AbortController().signal })).rejects.toBeInstanceOf(ArchiveReadError);
    });
});
describe('public archive images', () => {
    it('does not fall back to live media when no public adapter exists', async () => {
        const h = setup();
        await h.controller.load();
        await h.controller.openMedia(id(12), id(30));
        expect(h.controller.read()).toMatchObject({ media: null, mediaPhase: 'unavailable' });
        expect(h.calls).toHaveLength(1);
    });
    it('requires membership of current public page before opening media', async () => {
        const read = vi.fn(), h = setup({ read });
        await h.controller.load();
        await h.controller.openMedia(id(999), id(30));
        expect(read).not.toHaveBeenCalled();
    });
    it('revokes an image on context withdrawal and rejects a late blob', async () => {
        const allocate = vi.fn(() => 'blob:public'), revoke = vi.fn();
        vi.stubGlobal('URL', class extends URL {
            static createObjectURL = allocate;
            static revokeObjectURL = revoke;
        });
        const held = deferred<Awaited<ReturnType<ArchiveMediaPort['read']>>>();
        let late = false;
        const h = setup({ read: async (r) => late ? held.promise : { ...r, blob: imageBlob() } });
        await h.controller.load();
        await h.controller.openMedia(id(12), id(30));
        expect(h.controller.read().media?.url).toBe('blob:public');
        late = true;
        const pending = h.controller.openMedia(id(12), id(30));
        h.context.setEventId(null);
        held.resolve({ eventId, archiveRevision: 7, postId: id(12), mediaId: id(30), blob: imageBlob() });
        await pending;
        expect(revoke).toHaveBeenCalledWith('blob:public');
        expect(allocate).toHaveBeenCalledTimes(1);
        expect(h.controller.read().media).toBeNull();
    });
    it.each(['foreign', 'html', 'empty', 'oversize'])('rejects %s image results', async (kind) => {
        const h = setup({ read: async (r) => ({ ...r, eventId: kind === 'foreign' ? id(99) : r.eventId, blob: kind === 'html' ? new Blob(['x'], { type: 'text/html' }) : kind === 'empty' ? new Blob([], { type: 'image/png' }) : kind === 'oversize' ? new Blob([new Uint8Array(10 * 1024 * 1024 + 1)], { type: 'image/png' }) : imageBlob() }) });
        await h.controller.load();
        await h.controller.openMedia(id(12), id(30));
        expect(h.controller.read()).toMatchObject({ media: null, mediaPhase: 'unavailable' });
    });
});
it('does not reintroduce an invalidated deleted post from a stale public response', async () => {
    const h = setup();
    await h.controller.load();
    h.controller.invalidatePost(eventId,id(12));
    await h.controller.load();
    expect(h.controller.read()).toMatchObject({ phase: 'error', data: null, failure: 'changed' });
    h.setRead(async () => page(true));
    await h.controller.load();
    expect(h.controller.read().data?.feed.items[0]?.post_id).toBe(id(11));
});
it('drops a late public response on an account change before its next read', async () => {
    const session = createMemorySession(), context = createArchiveContext(eventId, session), held = deferred<unknown>(), controller = createArchiveController({ context, reader: { read: () => held.promise } });
    try {
        const pending = controller.load();
        session.replace(0, { userId: id(50), eventId, accessToken: 'PRIVATE_TOKEN' });
        held.resolve(page());
        await pending;
        expect(controller.read()).toMatchObject({ phase: 'idle', data: null });
    }
    finally {
        controller.dispose();
        context.dispose();
    }
});
it('accepts fractional-second ordering across pages using the FE27 ordering rule', async () => {
    const h = setup(), first = page();
    first.feed.items[0]!.created_at = '2026-11-07T14:00:00.1Z';
    h.setRead(async () => first);
    await h.controller.load();
    const second = page(true);
    second.feed.items[0]!.created_at = '2026-11-07T14:00:00Z';
    h.setRead(async () => second);
    await h.controller.load(true);
    expect(h.controller.read().phase).toBe('ready');
});

it('fences deletion notifications arriving during a pending read and rejects foreign-event notices',async()=>{
    const h=setup(),held=deferred<unknown>();h.setRead(()=>held.promise);const pending=h.controller.load();
    h.controller.invalidatePost(id(99),id(12));expect(h.controller.read().phase).toBe('loading');
    h.controller.invalidatePost(eventId,id(12));expect(h.controller.read()).toMatchObject({phase:'error',data:null,failure:'changed'});
    held.resolve(page());await pending;expect(h.controller.read().data).toBeNull();
    h.setRead(async()=>page());await h.controller.load();expect(h.controller.read()).toMatchObject({phase:'error',data:null,failure:'changed'});
});

it('keeps the last ordering bound across a permitted empty intermediate page',async()=>{
    const h=setup();await h.controller.load();const empty=page(true);empty.feed.items=[];empty.feed.next_cursor='page-three';h.setRead(async()=>empty);await h.controller.load(true);
    const bad=page(true);bad.feed.items[0]!.created_at='2026-11-07T16:00:00Z';h.setRead(async()=>bad);await h.controller.load(true);expect(h.controller.read()).toMatchObject({phase:'error',data:null,failure:'invalid'});
});
