import type { SessionController } from '../../../auth/model/session';
import type { PrivacyScope } from '../../../../shared/query/privacy';
import { TransportError } from '../../../../shared/api/errors';
import { immutable } from '../../../../world/entities/replicated';
import { actions, checked, detail, listOperation, listSegment, operation, receiptDecoder, row, rowId, rowKey, uuid, type Binding, type Content, type EvidenceDetail, type Lane, type Page, type Receipt, type Row } from './contract';
export interface ModeratorLease {
    readonly scope: PrivacyScope;
    readonly role: 'MODERATOR' | 'OPERATOR';
    readonly decisionId: string;
    readonly expiresAt: number;
}
/** Integrator supplies a current server-qualified decision, never an Auth/global-role guess. */
export interface ModeratorAccess {
    read(): ModeratorLease | null;
    subscribe(listener: () => void): () => void;
}
export const unavailableModeratorAccess: ModeratorAccess = { read: () => null, subscribe: () => () => {
    } };
/** Authorized REPORT media adapter. No URLs, tokens or unprotected img src accepted here. */
export interface EvidencePort {
    open(options: {
        scope: PrivacyScope;
        reportId: string;
        mediaId: string;
        signal: AbortSignal;
    }): Promise<{
        mediaId: string;
        blob: Blob;
    }>;
}
export interface ModerationOptions {
    auth: Pick<SessionController, 'session' | 'privacy' | 'request'>;
    access: ModeratorAccess;
    lane: Lane;
    memberId?: string;
    evidence?: EvidencePort;
    now?: () => number;
    newKey?: () => string;
}
type Phase = 'idle' | 'loading' | 'ready' | 'failed' | 'conflict' | 'sending' | 'pending' | 'unknown' | 'checking' | 'applied' | 'denied';
export interface ModerationState {
    readonly access: boolean;
    readonly phase: Phase;
    readonly rows: readonly Row[];
    readonly cursor: string | null;
    readonly selected: Row | null;
    readonly detail: EvidenceDetail | null;
    readonly evidenceUrl: string | null;
    readonly reason: string;
    readonly action: string;
    readonly confirm: boolean;
    readonly message: string;
    readonly receipt: string | null;
    readonly epoch: number;
}
export function createModeration(options: ModerationOptions) {
    const { auth, access, lane } = options, now = options.now ?? Date.now;
    let state: ModerationState = immutable({ access: false, phase: 'idle', rows: [], cursor: null, selected: null, detail: null, evidenceUrl: null, reason: '', action: '', confirm: false, message: '', receipt: null, epoch: 0 });
    let disposed = false, unavailable = false, serial = 0, flight: AbortController | null = null, binding: (Binding & {
        targetRow: Row;
        action: string;
    }) | null = null, authority: ModeratorLease | null = null, timer: ReturnType<typeof setTimeout> | null = null;
    const listeners = new Set<() => void>();
    const emit = (patch: Partial<ModerationState>) => {
        if (disposed)
            return;
        state = immutable({ ...state, ...patch });
        for (const f of Array.from(listeners)) {
            try {
                f();
            }
            catch {
            }
        }
    };
    const sameScope = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId && a.generation === b.generation && a.policyGeneration === b.policyGeneration && a.gate === b.gate;
    function lease(): ModeratorLease | null {
        try {
            const a = access.read();
            return !disposed && !unavailable && (options.memberId === undefined || lane === 'members' && uuid(options.memberId))
                && a && ['MODERATOR', 'OPERATOR'].includes(a.role) && typeof a.decisionId === 'string' && a.decisionId.trim()
                && Number.isFinite(a.expiresAt) && a.expiresAt > now() && uuid(a.scope.userId) && uuid(a.scope.eventId)
                && auth.privacy.isCurrent(a.scope) && auth.session.isCurrent(a.scope) ? a : null;
        }
        catch {
            return null;
        }
    }
    function current(a: ModeratorLease | null = authority) {
        const b = lease();
        return !!a && !!b && sameScope(a.scope, b.scope) && a.role === b.role && a.decisionId === b.decisionId && a.expiresAt === b.expiresAt;
    }
    function revokeUrl() {
        if (state.evidenceUrl)
            URL.revokeObjectURL(state.evidenceUrl);
    }
    function scrub() {
        serial++;
        flight?.abort();
        flight = null;
        binding = null;
        authority = null;
        if (timer)
            clearTimeout(timer);
        timer = null;
        revokeUrl();
        emit({ access: false, phase: 'denied', rows: [], cursor: null, selected: null, detail: null, evidenceUrl: null, reason: '', action: '', confirm: false, message: 'Current moderator access is required.', receipt: null, epoch: state.epoch + 1 });
    }
    function sync() {
        const a = lease();
        if (!current() || !a) {
            scrub();
            if (!a)
                return;
            authority = immutable(structuredClone(a));
            emit({ access: true, phase: 'idle', message: '' });
        }
        if (timer)
            clearTimeout(timer);
        timer = setTimeout(sync, Math.min(2147483647, Math.max(1, a!.expiresAt - now())));
    }
    let stopPrivacy = () => {}, stopAccess = () => {};
    function releaseSubscriptions() {
        const stops = [stopPrivacy, stopAccess];
        stopPrivacy = () => {};
        stopAccess = () => {};
        for (const stop of stops) {
            try { stop(); } catch { /* Release the remaining registrations. */ }
        }
    }
    try {
        stopPrivacy = auth.privacy.registerCleanup(scrub);
        stopAccess = access.subscribe(sync);
        sync();
    } catch {
        // Without both withdrawal observers, a readable role is not authority.
        unavailable = true;
        releaseSubscriptions();
        scrub();
    }
    function allowed() {
        if (!current()) {
            scrub();
            return false;
        }
        return true;
    }
    const locked = () => ['loading', 'sending', 'pending', 'unknown', 'checking', 'applied'].includes(state.phase);
    function begin() {
        flight?.abort();
        const c = new AbortController();
        flight = c;
        const n = ++serial;
        return { c, n, a: authority! };
    }
    const alive = (x: ReturnType<typeof begin>) => !disposed && serial === x.n && !x.c.signal.aborted && current(x.a);
    async function readPage(x: ReturnType<typeof begin>, cursor?: string) {
        const path = `/api/concert/events/${x.a.scope.eventId}/admin/${listSegment(lane)}`;
        return auth.privacy.run(x.a.scope, async ({ signal }) => (await auth.request({ path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: listOperation(lane), query: { limit: 20, ...(lane === 'members' && options.memberId ? { user_id: options.memberId } : {}), ...(cursor ? { cursor } : {}) }, signal, decode: (v, ctx) => {
                if (ctx.path !== path || ctx.status !== 200 || !sameScope(x.a.scope, { ...x.a.scope, ...ctx.scope }))
                    throw new Error('Invalid list context');
                const p = checked<Page<unknown>>(lane === 'reports' ? 'ReportList' : lane === 'members' ? 'UserReviewList' : 'ContentReviewList', v), rows = p.items.map(v => row(lane, v, x.a.scope.eventId!));
                if (lane === 'members' && options.memberId && rows.some(r => rowId(r) !== options.memberId))
                    throw new Error('Foreign member');
                if (new Set(rows.map(rowKey)).size !== rows.length)
                    throw new Error('Duplicate rows');
                return immutable({ items: rows, next_cursor: p.next_cursor });
            } })).data, x.c.signal);
    }
    function failure(error: unknown, sent: boolean) {
        const t = error instanceof TransportError ? error : null, reason = t?.server?.reason;
        if (t?.status === 401 || t?.status === 403) {
            scrub();
            return;
        }
        const conflict = reason === 'VERSION_CONFLICT', unknown = sent && (!t || t.uncertain || !t.status || t.status >= 500 || ['IDEMPOTENCY_CONFLICT', 'IDEMPOTENCY_EXPIRED'].includes(reason ?? ''));
        emit({ phase: unknown ? 'unknown' : conflict ? 'conflict' : 'failed', confirm: false, ...(!sent ? { rows: [], cursor: null, selected: null, detail: null } : {}), message: unknown ? 'Outcome unknown. Check this command before taking another action.' : conflict ? 'The record changed. Refresh and review the current version before confirming again.' : 'The request failed. No applied action has been confirmed.' });
    }
    async function load(next = false) {
        if (!allowed() || ['sending', 'pending', 'unknown', 'checking'].includes(state.phase))
            return;
        const cursor = next ? state.cursor : null;
        if (next && !cursor)
            return;
        const x = begin();
        binding = null;
        revokeUrl();
        emit({ phase: 'loading', rows: [], selected: null, detail: null, evidenceUrl: null, confirm: false, action: '', message: '' });
        try {
            const p = await readPage(x, cursor ?? undefined);
            if (alive(x))
                emit({ phase: 'ready', rows: p.items, cursor: p.next_cursor, receipt: null, ...(lane === 'members' && options.memberId ? { selected: p.items[0] ?? null, epoch: state.epoch + 1 } : {}) });
        }
        catch (e) {
            if (alive(x))
                failure(e, false);
        }
    }
    async function select(key: string) {
        if (!allowed() || locked() || state.phase === 'failed' && !!binding)
            return;
        const target = state.rows.find(r => rowKey(r) === key);
        if (!target)
            return;
        const x = begin();
        revokeUrl();
        binding = null;
        emit({ selected: target, detail: null, evidenceUrl: null, reason: '', action: '', confirm: false, receipt: null, message: '', phase: lane === 'reports' ? 'loading' : 'ready', epoch: state.epoch + 1 });
        if (lane !== 'reports')
            return;
        const path = `/api/concert/events/${x.a.scope.eventId}/admin/reports/${rowId(target)}`;
        try {
            const d = await auth.privacy.run(x.a.scope, async ({ signal }) => (await auth.request({ path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertAdminReport', signal, decode: (v, ctx) => {
                    if (ctx.path !== path || ctx.status !== 200)
                        throw new Error('Invalid detail context');
                    return detail(v, rowId(target), x.a.scope.eventId!);
                } })).data, x.c.signal);
            if (!alive(x))
                return;
            if (d.report.revision !== target.revision) {
                emit({ phase: 'conflict', selected: null, detail: null, message: 'The report changed. Refresh the queue before reviewing it.' });
                return;
            }
            emit({ phase: 'ready', detail: d });
        }
        catch (e) {
            if (alive(x))
                failure(e, false);
        }
    }
    async function evidence(mediaId: string) {
        if (!allowed() || locked() || !state.detail?.evidence_media_ids.includes(mediaId))
            return;
        revokeUrl();
        emit({ evidenceUrl: null, message: '' });
        if (!options.evidence) {
            emit({ message: 'Evidence is unavailable. No public fallback is used.' });
            return;
        }
        const x = begin(), reportId = state.detail.report.report_id;
        try {
            const result = await options.evidence.open({ scope: x.a.scope, reportId, mediaId, signal: x.c.signal });
            if (!alive(x))
                return;
            if (result.mediaId !== mediaId || result.blob.size < 1 || result.blob.size > 10 * 1024 * 1024 || !['image/png', 'image/jpeg', 'image/webp'].includes(result.blob.type))
                throw new Error('Unverified evidence');
            emit({ evidenceUrl: URL.createObjectURL(result.blob), message: 'Private evidence opened for this review only.' });
        }
        catch (e) {
            if (alive(x))
                failure(e, false);
        }
    }
    function draft(action: string, reason: string) {
        if (!allowed() || locked() || (state.phase === 'conflict' || state.phase === 'failed' && !!binding))
            return;
        emit({ action, reason, confirm: false, message: '' });
    }
    function review() {
        if (!allowed() || locked() || (state.phase === 'conflict' || state.phase === 'failed' && !!binding) || !state.selected || !actions(state.selected).includes(state.action) || !state.reason.trim() || Array.from(state.reason).length > (lane === 'reports' ? 2000 : 500) || lane === 'reports' && !state.detail)
            return;
        emit({ confirm: true });
    }
    async function accept(r: Receipt, x: ReturnType<typeof begin>) {
        const b = binding;
        if (!b)
            return;
        if (state.receipt && state.receipt !== r.receipt_id)
            throw new Error('Receipt identity changed');
        emit({ receipt: r.receipt_id, confirm: false });
        if (r.state === 'RETIRED' || Date.parse(r.expires_at) <= now()) {
            emit({ phase: 'unknown', message: 'Receipt retired or expired. Do not repeat this action.' });
            return;
        }
        if (r.state === 'PENDING') {
            emit({ phase: 'pending', message: 'Action pending. No applied result has been confirmed.' });
            return;
        }
        if (r.state === 'FAILED') {
            emit({ phase: r.failure_reason === 'VERSION_CONFLICT' ? 'conflict' : 'failed', message: 'The server recorded failure. Refresh before starting a new action.' });
            return;
        }
        // A successful receipt is metadata; re-read the owning review projection before claiming applied.
        const p = await readPage(x);
        if (!alive(x))
            return;
        const found = p.items.find(v => rowKey(v) === rowKey(b.targetRow));
        const expected = b.action === 'SUSPEND' ? 'SUSPENDED' : b.action === 'UNSUSPEND' ? 'ACTIVE' : b.action === 'HIDE' ? 'HIDDEN' : b.action === 'RESTORE' ? 'VISIBLE' : b.action;
        if (!found || (r.resource_revision ?? 0) <= b.targetRow.revision || found.revision < (r.resource_revision ?? Infinity) || found.state !== expected) {
            emit({ phase: 'unknown', message: 'Receipt found, but the current record does not confirm application. No repeat was sent.' });
            return;
        }
        emit({ phase: 'applied', rows: p.items, cursor: p.next_cursor, selected: found, detail: null, reason: '', action: '', message: 'The current server record confirms this action.' });
    }
    async function submit() {
        if (!allowed() || locked() || !state.confirm || !state.selected)
            return;
        const selected = state.selected, action = state.action, reason = state.reason;
        if (!actions(selected).includes(action) || !reason.trim() || (state.phase === 'conflict' || state.phase === 'failed' && !!binding))
            return;
        const key = (options.newKey ?? (() => crypto.randomUUID()))();
        if (!uuid(key))
            return;
        const expected_revision = selected.revision, body = lane === 'reports' ? { expected_revision, decision: action, note: reason } : lane === 'members' ? { expected_revision, action, expires_at: null, reason } : { expected_revision, target_type: (selected as Content).target_type, target_id: rowId(selected), action, reason };
        checked(lane === 'reports' ? 'ResolveReport' : lane === 'members' ? 'ModerateUser' : 'ModerateContent', body);
        const x = begin(), op = operation(lane);
        binding = { scope: x.a.scope, operation: op, key, target: rowId(selected), targetRow: selected, action };
        const path = `/api/concert/events/${x.a.scope.eventId}/admin/${lane === 'reports' ? `reports/${rowId(selected)}/decision` : lane === 'members' ? `users/${rowId(selected)}/moderation` : 'content/moderation'}`;
        revokeUrl();
        emit({ phase: 'sending', evidenceUrl: null, confirm: false, message: 'Sending one moderation command.' });
        try {
            const result = await auth.privacy.run(x.a.scope, async ({ signal }) => (await auth.request<Row | Receipt>({ path, method: 'POST', access: 'protected', errorFamily: 'domain', domainOperation: op, idempotencyKey: key, body: { json: body }, signal, decode: (v, ctx) => {
                    if (ctx.status === 202)
                        return receiptDecoder(binding!, path)(v, ctx);
                    if (ctx.path !== path || ctx.status !== 200)
                        throw new Error('Invalid action context');
                    const r = row(lane, v, x.a.scope.eventId!);
                    if (rowKey(r) !== rowKey(selected) || r.revision <= selected.revision || r.state !== (action === 'SUSPEND' ? 'SUSPENDED' : action === 'UNSUSPEND' ? 'ACTIVE' : action === 'HIDE' ? 'HIDDEN' : action === 'RESTORE' ? 'VISIBLE' : action))
                        throw new Error('Unconfirmed action');
                    return r;
                } })).data, x.c.signal);
            if (!alive(x))
                return;
            if ('receipt_id' in result)
                await accept(result, x);
            else
                emit({ phase: 'applied', rows: state.rows.map(r => rowKey(r) === rowKey(result) ? result : r), selected: result, detail: null, reason: '', action: '', message: 'The server confirmed this action.' });
        }
        catch (e) {
            if (alive(x))
                failure(e, true);
        }
    }
    async function checkStatus() {
        if (!allowed() || !binding || !['pending', 'unknown'].includes(state.phase))
            return;
        const b = binding, x = begin(), path = `/api/concert/events/${x.a.scope.eventId}/commands`;
        emit({ phase: 'checking', message: 'Checking the original command.' });
        try {
            const r = await auth.privacy.run(x.a.scope, async ({ signal }) => (await auth.request({ path, method: 'GET', access: 'protected', errorFamily: 'domain', domainOperation: 'getConcertCommandStatus', query: { operation_id: b.operation, idempotency_key: b.key }, signal, decode: receiptDecoder(b, path) })).data, x.c.signal);
            if (alive(x))
                await accept(r, x);
        }
        catch (e) {
            if (alive(x))
                failure(e, true);
        }
    }
    return { lane, read: () => state, subscribe(f: () => void) {
            listeners.add(f);
            return () => {
                listeners.delete(f);
            };
        }, load, select, evidence, draft, review, submit, checkStatus, cancel() {
            if (allowed() && !locked())
                emit({ confirm: false });
        }, dispose() {
            if (disposed)
                return;
            scrub();
            disposed = true;
            releaseSubscriptions();
            listeners.clear();
        } };
}
export type ModerationController = ReturnType<typeof createModeration>;
