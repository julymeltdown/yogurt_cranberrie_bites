import { receiptDecoder, type Binding } from '../../../party-authoring/model/contract';
import { uuid, type Receipt } from '../../../party-discovery/model/contract';
import { adminValue } from './validate';
import { immutable } from '../../../../world/entities/replicated';
export { uuid, type Receipt, type Binding };
export const checked = <T>(name: string, value: unknown): T => immutable(adminValue<T>(`Domain_admin_${name}`, value));
export { receiptDecoder };
export interface Report {
    report_id: string;
    event_id: string;
    target_type: 'USER' | 'POST' | 'COMMENT' | 'PARTY' | 'GIVEAWAY' | 'MURAL';
    target_id: string;
    state: 'OPEN' | 'IN_REVIEW' | 'RESOLVED' | 'DISMISSED';
    revision: number;
    created_at: string;
}
/** Reporter identity is deliberately excluded from retained state and all feature UI. */
export interface EvidenceDetail {
    report: Report;
    reason: string;
    text: string;
    evidence_media_ids: readonly string[];
}
export function detail(value: unknown, id: string, event: string): EvidenceDetail {
    const d = checked<EvidenceDetail & {
        reporter_id: string;
    }>('ReportDetail', value);
    if (d.report.report_id !== id || d.report.event_id !== event)
        throw new Error('Invalid report context');
    return immutable({ report: d.report, reason: d.reason, text: d.text, evidence_media_ids: d.evidence_media_ids });
}
export interface Member {
    user_id: string;
    profile: {
        user_id: string;
        display_name: string;
    };
    state: 'ACTIVE' | 'SUSPENDED' | 'DELETION_PENDING' | 'DELETED';
    revision: number;
}
export interface Content {
    target_type: 'POST' | 'COMMENT' | 'PARTY' | 'GIVEAWAY' | 'INTRO';
    target_id: string;
    state: 'VISIBLE' | 'HIDDEN' | 'DELETED';
    revision: number;
}
export type Row = Report | Member | Content;
export type Lane = 'reports' | 'members' | 'content';
export interface Page<T> {
    items: readonly T[];
    next_cursor: string | null;
}
export const rowId = (r: Row) => 'report_id' in r ? r.report_id : 'user_id' in r ? r.user_id : r.target_id;
export const rowKey = (r: Row) => 'target_type' in r && !('report_id' in r) ? `${r.target_type}:${r.target_id}` : rowId(r);
export const rowSchema = (lane: Lane) => lane === 'reports' ? 'Report' : lane === 'members' ? 'UserReview' : 'ContentReview';
export function row(lane: Lane, value: unknown, event: string): Row {
    const r = checked<Row>(rowSchema(lane), value);
    if ('event_id' in r && r.event_id !== event || 'user_id' in r && r.user_id !== r.profile.user_id)
        throw new Error('Foreign review');
    if ('user_id' in r)
        return immutable({ ...r, profile: { user_id: r.profile.user_id, display_name: r.profile.display_name } });
    return r;
}
export const operation = (lane: Lane) => lane === 'reports' ? 'decideConcertReport' : lane === 'members' ? 'moderateConcertUser' : 'moderateConcertContent';
export const listOperation = (lane: Lane) => lane === 'reports' ? 'listConcertAdminReports' : lane === 'members' ? 'listConcertAdminUsers' : 'listConcertAdminContent';
export const listSegment = (lane: Lane) => lane === 'members' ? 'users' : lane;
export const actions = (r: Row): readonly string[] => 'report_id' in r ? ['OPEN', 'IN_REVIEW'].includes(r.state) ? ['RESOLVED', 'DISMISSED'] : []
    : 'user_id' in r ? r.state === 'ACTIVE' ? ['SUSPEND'] : r.state === 'SUSPENDED' ? ['UNSUSPEND'] : [] : r.state === 'VISIBLE' ? ['HIDE'] : r.state === 'HIDDEN' ? ['RESTORE'] : [];
