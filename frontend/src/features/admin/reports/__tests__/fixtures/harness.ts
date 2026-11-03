// TEST ONLY. Canonical Auth with deterministic HTTP candidate fixtures; no production role grant.
import { createSessionController } from '../../../../auth/model/session';
import { fixture, isolatedCookiePolicy, userId, eventId } from '../../../../auth/__tests__/fixtures/data';
import { createModeration, type EvidencePort, type ModeratorLease } from '../../model/controller';
import { operation, rowId, type Lane, type Row, type Report } from '../../model/contract';
export const id = (n: number) => '00000000-0000-4000-8000-' + String(n).padStart(12, '0');
export { userId, eventId };
export const report: Report = { report_id: id(300), event_id: eventId, target_type: 'POST', target_id: id(400), state: 'OPEN', revision: 1, created_at: '2026-11-03T00:00:00Z' };
export const privateDetail = { report, reporter_id: id(999), reason: 'PRIVACY', text: 'Private test statement', evidence_media_ids: [id(500)] };
export const rows = (lane: Lane): Row[] => lane === 'reports' ? [structuredClone(report)] : lane === 'members' ? [{ user_id: id(400), profile: { ...fixture().participant.profile, user_id: id(400), display_name: 'Review member' }, state: 'ACTIVE', revision: 1 }] : [{ target_type: 'POST', target_id: id(400), state: 'VISIBLE', revision: 1 }];
export function harness(lane: Lane = 'reports', evidence?: EvidencePort) {
    const requests: Request[] = [], listeners = new Set<() => void>();
    let keyCounter=799;
    let handler: ((r: Request) => Response | Promise<Response> | null) | null = null, data = rows(lane);
    const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async (input) => {
            const r = input as Request;
            requests.push(r.clone());
            const overridden = handler?.(r);
            if (overridden)
                return overridden;
            const path = new URL(r.url).pathname;
            if (r.method === 'POST') {
                const body = await r.clone().json() as {
                    action?: string;
                    decision?: string;
                };
                const action = body.action ?? body.decision;
                const state = action === 'SUSPEND' ? 'SUSPENDED' : action === 'UNSUSPEND' ? 'ACTIVE' : action === 'HIDE' ? 'HIDDEN' : action === 'RESTORE' ? 'VISIBLE' : action;
                data = data.map(v => ({ ...v, revision: v.revision + 1, state }) as Row);
                return Response.json(data[0]);
            }
            if (path.endsWith('/' + report.report_id))
                return Response.json({ ...privateDetail, report: data[0] });
            return Response.json({ items: data, next_cursor: null });
        } });
    auth.session.replace(0, { userId, eventId, accessToken: 'fixture-moderator-memory-only' });
    let grant: ModeratorLease | null = { scope: auth.privacy.capture(), role: 'MODERATOR', decisionId: 'test-only-server-decision', expiresAt: Date.now() + 60000 };
    const access = { read: () => grant, subscribe(fn: () => void) {
            listeners.add(fn);
            return () => {
                listeners.delete(fn);
            };
        } };
    const controller = createModeration({ auth, access, lane, ...(evidence ? { evidence } : {}), newKey: () => id(++keyCounter) });
    return { auth, access, controller, requests, override(fn: typeof handler) {
            handler = fn;
        }, setRows(value: Row[]) {
            data = value;
        }, getRows: () => data, grant: () => grant,
        setGrant(value: ModeratorLease | null) {
            grant = value;
            [...listeners].forEach(fn => fn());
        },
        receipt(state = 'PENDING', changes: Record<string, unknown> = {}) {
            return { receipt_id: id(700+keyCounter-800), event_id: eventId, actor_id: userId, operation_id: operation(lane), idempotency_key: requests.filter(r=>r.method==='POST').at(-1)?.headers.get('Idempotency-Key')??id(800), state, resource_id: state === 'SUCCEEDED' ? rowId(data[0]!) : null, resource_revision: state === 'SUCCEEDED' ? 2 : null, failure_reason: state === 'FAILED' ? 'DEPENDENCY_UNAVAILABLE' : null, result_status: state === 'SUCCEEDED' ? 200 : null, created_at: '2026-11-04T00:00:00Z', expires_at: '2027-11-04T00:00:00Z', ...changes };
        },
        dispose() {
            controller.dispose();
            auth.dispose();
        } };
}
