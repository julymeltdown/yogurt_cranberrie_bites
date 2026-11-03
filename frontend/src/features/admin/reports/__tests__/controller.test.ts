// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { harness, id, privateDetail, report, eventId, userId, rows } from './fixtures/harness';
import { checked, rowKey, type Lane } from '../model/contract';
import { createModeration } from '../model/controller';
import { deferred } from '../../../auth/__tests__/fixtures/data';
const owned: ReturnType<typeof harness>[] = [];
const setup = (lane: Lane = 'reports') => {
    const h = harness(lane);
    owned.push(h);
    return h;
};
afterEach(() => {
    owned.splice(0).forEach(h => h.dispose());
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.useRealTimers();
});
async function prepare(h: ReturnType<typeof harness>, action?: string) {
    await h.controller.load();
    await h.controller.select(rowKey(h.controller.read().rows[0]!));
    h.controller.draft(action ?? (h.controller.lane === 'reports' ? 'RESOLVED' : h.controller.lane === 'members' ? 'SUSPEND' : 'HIDE'), 'Reviewed factual reason');
    h.controller.review();
}
const error = (status: number, reason: string) => Response.json({ code: 'FIXTURE', message: 'PRIVATE SERVER DIAGNOSTIC', details: { reason } }, { status });
describe('fail closed moderator authority and private reads', () => {
    it.each(['missing', 'role', 'event', 'identity', 'expired', 'policy'] as const)('rejects %s authority without a request', async (kind) => {
        const h = setup(), g = h.grant()!;
        h.setGrant(kind === 'missing' ? null : kind === 'role' ? { ...g, role: 'MEMBER' as never } : kind === 'expired' ? { ...g, expiresAt: 0 } : { ...g, scope: { ...g.scope, ...(kind === 'event' ? { eventId: id(1) } : kind === 'identity' ? { userId: id(2) } : { policyGeneration: 99 }) } });
        await h.controller.load();
        expect(h.controller.read().access).toBe(false);
        expect(h.requests).toHaveLength(0);
    });
    it('accepts the existing OPERATOR role only for this event', async () => {
        const h = setup();
        h.setGrant({ ...h.grant()!, role: 'OPERATOR' });
        await h.controller.load();
        expect(h.controller.read().rows).toHaveLength(1);
    });
    it('removes reporter identity from retained details and never reads evidence automatically', async () => {
        const h = setup();
        await prepare(h);
        expect(h.controller.read().detail?.text).toBe(privateDetail.text);
        expect(JSON.stringify(h.controller.read())).not.toContain(privateDetail.reporter_id);
        expect(h.requests).toHaveLength(2);
        expect(h.requests.every(r => r.cache === 'no-store')).toBe(true);
        await h.controller.evidence(privateDetail.evidence_media_ids[0]!);
        expect(h.controller.read().evidenceUrl).toBeNull();
        expect(h.controller.read().message).toContain('unavailable');
    });
    it.each(['logout', 'event', 'policy', 'permission'] as const)('scrubs rows/reason/confirmation on %s synchronously', async (kind) => {
        const h = setup();
        await prepare(h);
        if (kind === 'logout')
            h.auth.session.replace(h.auth.session.read().generation, null);
        if (kind === 'event')
            h.auth.session.replace(h.auth.session.read().generation, { userId, eventId: id(999), accessToken: 'next-fixture-token' });
        if (kind === 'policy')
            h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'block');
        if (kind === 'permission')
            h.setGrant(null);
        expect(h.controller.read()).toMatchObject({ access: false, rows: [], selected: null, detail: null, reason: '', confirm: false, receipt: null });
    });
    it('expires a qualified role lease without waiting for the next click', async () => {
        vi.useFakeTimers();
        const h = setup();
        await prepare(h);
        h.setGrant({ ...h.grant()!, expiresAt: Date.now() + 10 });
        await vi.advanceTimersByTimeAsync(11);
        expect(h.controller.read().rows).toEqual([]);
        expect(h.controller.read().access).toBe(false);
    });
    it('drops a late list response after a permission decision changes', async () => {
        const h = setup(), held = deferred<Response>();
        h.override(() => held.promise);
        const pending = h.controller.load();
        h.setGrant(null);
        held.resolve(Response.json({ items: [report], next_cursor: null }));
        await pending;
        expect(h.controller.read().rows).toEqual([]);
    });
    it('drops a late report detail after logout', async () => {
        const h = setup();
        await h.controller.load();
        const held = deferred<Response>();
        h.override(() => held.promise);
        const pending = h.controller.select(report.report_id);
        h.auth.session.replace(h.auth.session.read().generation, null);
        held.resolve(Response.json(privateDetail));
        await pending;
        expect(h.controller.read().detail).toBeNull();
    });
    it('scrubs former private records on 403 and does not expose server text', async () => {
        const h = setup();
        await prepare(h);
        h.override(() => error(403, 'PERMISSION_DENIED'));
        await h.controller.load();
        expect(h.controller.read()).toMatchObject({ access: false, rows: [], detail: null, reason: '' });
        expect(JSON.stringify(h.controller.read())).not.toContain('PRIVATE SERVER');
    });
    it('rejects stale detail and foreign or duplicated list rows', async () => {
        const h = setup();
        await h.controller.load();
        h.override(() => Response.json({ ...privateDetail, report: { ...report, revision: 2 } }));
        await h.controller.select(report.report_id);
        expect(h.controller.read().phase).toBe('conflict');
        expect(h.controller.read().detail).toBeNull();
        h.override(() => Response.json({ items: [{ ...report, event_id: id(4) }], next_cursor: null }));
        await h.controller.load();
        expect(h.controller.read().rows).toEqual([]);
    });
});
describe('explicit revision/reason and truthful commands', () => {
    it.each(['reports', 'members', 'content'] as const)('binds exact %s command actor/event/target/revision/key with no replay', async (lane) => {
        const h = setup(lane);
        await prepare(h);
        await h.controller.submit();
        expect(h.controller.read().phase).toBe('applied');
        const writes = h.requests.filter(r => r.method === 'POST');
        expect(writes).toHaveLength(1);
        const r = writes[0]!, body = await r.clone().json();
        expect(r.headers.get('Idempotency-Key')).toBe(id(800));
        expect(new URL(r.url).pathname).toContain('/events/' + eventId + '/admin/');
        expect(body.expected_revision).toBe(1);
        expect(body.note ?? body.reason).toBe('Reviewed factual reason');
        if (lane === 'content')
            expect(body.target_id).toBe(id(400));
        else
            expect(new URL(r.url).pathname).toContain(lane === 'reports' ? report.report_id : id(400));
        await h.controller.submit();
        expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1);
    });
    it('requires explicit review and confirmation; cancel has no command side effect', async () => {
        const h = setup();
        await h.controller.load();
        await h.controller.select(report.report_id);
        h.controller.draft('RESOLVED', 'reason');
        await h.controller.submit();
        expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(0);
        h.controller.review();
        h.controller.cancel();
        await h.controller.submit();
        expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(0);
    });
    it('suppresses duplicate submissions while pending and uses only GET status recovery', async () => {
        const h = setup();
        await prepare(h);
        h.override(r => r.method === 'POST' ? Response.json(h.receipt(), { status: 202 }) : null);
        await Promise.all([h.controller.submit(), h.controller.submit()]);
        expect(h.controller.read().phase).toBe('pending');
        await h.controller.submit();
        expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1);
        h.override(r => new URL(r.url).pathname.endsWith('/commands') ? Response.json(h.receipt('FAILED')) : null);
        await h.controller.checkStatus();
        expect(h.controller.read().phase).toBe('failed');
        expect(h.controller.read().reason).toBe('Reviewed factual reason');
        expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1);
    });
    it.each(['network', 'downstream', 'receipt conflict', 'stale success', 'wrong actor', 'wrong target'] as const)('never claims applied for %s', async (kind) => {
        const h = setup();
        await prepare(h);
        h.override(() => {
            if (kind === 'network')
                throw new Error('private connection diagnostic');
            if (kind === 'downstream')
                return error(503, 'DEPENDENCY_UNAVAILABLE');
            if (kind === 'receipt conflict')
                return error(409, 'IDEMPOTENCY_CONFLICT');
            if (kind === 'stale success')
                return Response.json(report);
            return Response.json(h.receipt('PENDING', kind === 'wrong actor' ? { actor_id: id(900) } : { resource_id: id(901) }), { status: 202 });
        });
        await h.controller.submit();
        expect(h.controller.read().phase).toBe('unknown');
        expect(h.controller.read().reason).toBe('Reviewed factual reason');
        await h.controller.submit();
        expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1);
    });
    it('requires a current row after a successful receipt before claiming applied', async () => {
        const h = setup();
        await prepare(h);
        h.override(r => r.method === 'POST' ? Response.json(h.receipt(), { status: 202 }) : new URL(r.url).pathname.endsWith('/commands') ? Response.json(h.receipt('SUCCEEDED')) : null);
        await h.controller.submit();
        await h.controller.checkStatus();
        expect(h.controller.read().phase).toBe('unknown');
    });
    it('does not convert partial/failed receipt into success', async () => {
        const h = setup();
        await prepare(h);
        h.override(r => r.method === 'POST' ? Response.json(h.receipt(), { status: 202 }) : new URL(r.url).pathname.endsWith('/commands') ? Response.json(h.receipt('FAILED')) : null);
        await h.controller.submit();
        await h.controller.checkStatus();
        expect(h.controller.read().phase).toBe('failed');
        expect(h.controller.read().selected?.state).toBe('OPEN');
    });
    it('rejects stale versions and preserves reason pending an explicit refresh', async () => {
        const h = setup();
        await prepare(h);
        h.override(() => error(409, 'VERSION_CONFLICT'));
        await h.controller.submit();
        expect(h.controller.read().phase).toBe('conflict');
        expect(h.controller.read().reason).toBe('Reviewed factual reason');
        h.controller.review();
        expect(h.controller.read().confirm).toBe(false);
    });
    it('keeps deleted content and terminal members nonactionable', async () => {
        const h = setup('content');
        h.setRows(rows('content').map(r => ({ ...r, state: 'DELETED' }) as typeof r));
        await prepare(h, 'RESTORE');
        expect(h.controller.read().confirm).toBe(false);
    });
    it('validates C04 annotation-bearing moderation bodies and closed field limits', () => {
        expect(() => checked('ModerateUser', { expected_revision: 1, action: 'SUSPEND', expires_at: null, reason: 'reason' })).not.toThrow();
        expect(() => checked('ModerateContent', { expected_revision: 1, target_type: 'POST', target_id: id(1), action: 'HIDE', reason: 'reason', reporter_id: id(99) })).toThrow();
    });
    it('fails closed when no qualified port is composed', async () => {
        const h = setup();
        const c = createModeration({ auth: h.auth, lane: 'reports', access: { read: () => null, subscribe: () => () => {
                } } });
        await c.load();
        expect(c.read().access).toBe(false);
        c.dispose();
    });
});
describe('private evidence, direct member entry and command recovery', () => {
    it('opens authorized evidence only on demand and revokes its blob on permission loss', async () => {
        const open = vi.fn(async () => ({ mediaId: id(500), blob: new Blob(['private'], { type: 'image/png' }) })), revoke = vi.fn();
        vi.stubGlobal('URL', class extends URL {
            static createObjectURL = vi.fn(() => 'blob:private-evidence');
            static revokeObjectURL = revoke;
        });
        const h = harness('reports', { open });
        owned.push(h);
        await prepare(h);
        expect(open).not.toHaveBeenCalled();
        await h.controller.evidence(id(500));
        expect(h.controller.read().evidenceUrl).toBe('blob:private-evidence');
        expect(open.mock.calls[0]).toBeDefined();
        h.setGrant(null);
        expect(revoke).toHaveBeenCalledWith('blob:private-evidence');
        expect(h.controller.read().evidenceUrl).toBeNull();
    });
    it('drops a late evidence blob after permission loss without allocating a URL', async () => {
        const held = deferred<{
            mediaId: string;
            blob: Blob;
        }>(), allocate = vi.fn();
        vi.stubGlobal('URL', class extends URL {
            static createObjectURL = allocate;
            static revokeObjectURL = vi.fn();
        });
        const h = harness('reports', { open: () => held.promise });
        owned.push(h);
        await prepare(h);
        const pending = h.controller.evidence(id(500));
        h.setGrant(null);
        held.resolve({ mediaId: id(500), blob: new Blob(['x'], { type: 'image/png' }) });
        await pending;
        expect(allocate).not.toHaveBeenCalled();
        expect(h.controller.read().detail).toBeNull();
    });
    it.each(['wrong identity', 'html', 'oversize'])('rejects %s evidence without an image URL', async (kind) => {
        const h = harness('reports', { open: async () => ({ mediaId: kind === 'wrong identity' ? id(99) : id(500), blob: new Blob([kind === 'oversize' ? new Uint8Array(10 * 1024 * 1024 + 1) : 'x'], { type: kind === 'html' ? 'text/html' : 'image/png' }) }) });
        owned.push(h);
        await prepare(h);
        await h.controller.evidence(id(500));
        expect(h.controller.read().evidenceUrl).toBeNull();
        expect(h.controller.read().phase).toBe('failed');
    });
    it('uses the frozen user_id filter for a direct member route and retains minimal fields', async () => {
        const h = setup('members'), c = createModeration({ auth: h.auth, access: h.access, lane: 'members', memberId: id(400) });
        try {
            await c.load();
            expect(new URL(h.requests[0]!.url).searchParams.get('user_id')).toBe(id(400));
            expect(c.read().selected).toMatchObject({ user_id: id(400) });
            expect(Object.keys((c.read().selected as {
                profile: object;
            }).profile)).toEqual(['user_id', 'display_name']);
        }
        finally {
            c.dispose();
        }
    });
    it('rejects an invalid direct member route without a private read', async () => {
        const h = setup('members'), c = createModeration({ auth: h.auth, access: h.access, lane: 'members', memberId: 'invalid' });
        await c.load();
        expect(c.read().access).toBe(false);
        expect(h.requests).toHaveLength(0);
        c.dispose();
    });
    it.each(['members', 'content'] as const)('supports the reverse %s action against the new revision', async (lane) => {
        const h = setup(lane);
        h.setRows(rows(lane).map(r => ({ ...r, state: lane === 'members' ? 'SUSPENDED' : 'HIDDEN', revision: 4 }) as typeof r));
        await prepare(h, lane === 'members' ? 'UNSUSPEND' : 'RESTORE');
        await h.controller.submit();
        expect(h.controller.read()).toMatchObject({ phase: 'applied', selected: { revision: 5, state: lane === 'members' ? 'ACTIVE' : 'VISIBLE' } });
    });
    it('confirms a succeeded receipt only after the current projection advances', async () => {
        const h = setup();
        await prepare(h);
        h.override(r => r.method === 'POST' ? Response.json(h.receipt(), { status: 202 }) : new URL(r.url).pathname.endsWith('/commands') ? Response.json(h.receipt('SUCCEEDED')) : null);
        await h.controller.submit();
        h.setRows([{ ...report, state: 'RESOLVED', revision: 2 }]);
        await h.controller.checkStatus();
        expect(h.controller.read().phase).toBe('applied');
        expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1);
    });
    it('rejects a replacement receipt identity for the same command', async () => {
        const h = setup();
        await prepare(h);
        h.override(r => r.method === 'POST' ? Response.json(h.receipt(), { status: 202 }) : new URL(r.url).pathname.endsWith('/commands') ? Response.json(h.receipt('FAILED', { receipt_id: id(701) })) : null);
        await h.controller.submit();
        await h.controller.checkStatus();
        expect(h.controller.read().phase).toBe('unknown');
    });
    it('does not resubmit a known failed command before refreshing its projection', async () => {
        const h = setup();
        await prepare(h);
        h.override(() => error(400, 'VALIDATION_FAILED'));
        await h.controller.submit();
        expect(h.controller.read().phase).toBe('failed');
        await h.controller.select(report.report_id);
        h.controller.review();
        await h.controller.submit();
        expect(h.requests.filter(r => r.method === 'POST')).toHaveLength(1);
        h.override(null);
        await h.controller.load();
        expect(h.controller.read().phase).toBe('ready');
    });
    it('scrubs a late command result after a new authority decision', async () => {
        const h = setup();
        await prepare(h);
        const held = deferred<Response>();
        h.override(() => held.promise);
        const pending = h.controller.submit();
        h.setGrant({ ...h.grant()!, decisionId: 'replacement' });
        held.resolve(Response.json({ ...report, state: 'RESOLVED', revision: 2 }));
        await pending;
        expect(h.controller.read()).toMatchObject({ phase: 'idle', rows: [], selected: null, reason: '', receipt: null });
    });
});
it('treats an unavailable role provider as denied without issuing a private read', async () => {
    const h = setup(), c = createModeration({ auth: h.auth, lane: 'reports', access: { read() {
                throw new Error('unavailable');
            }, subscribe: () => () => {
            } } });
    await c.load();
    expect(c.read().access).toBe(false);
    expect(h.requests).toHaveLength(0);
    c.dispose();
});

it('uses a fresh key only for a separately reviewed intent after refresh',async()=>{
    const h=setup('members');await prepare(h,'SUSPEND');await h.controller.submit();
    await prepare(h,'UNSUSPEND');await h.controller.submit();
    expect(h.requests.filter(r=>r.method==='POST').map(r=>r.headers.get('Idempotency-Key'))).toEqual([id(800),id(801)]);
    expect(h.controller.read()).toMatchObject({phase:'applied',selected:{state:'ACTIVE',revision:3}});
});
