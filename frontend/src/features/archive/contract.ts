import { parse, freeze, ordered, uuid } from '../event-feed/contract';
export { freeze, uuid, ordered };
export interface ArchivePost {
    readonly post_id: string;
    readonly text: string;
    readonly media_ids: readonly string[];
    readonly created_at: string;
}
export interface FinalMural {
    readonly snapshot_id: string;
    readonly event_id: string;
    readonly revision: number;
    readonly state: 'FROZEN';
    readonly width: 128;
    readonly height: 128;
    readonly palette: readonly string[];
    readonly pixels: readonly number[];
    readonly created_at: string;
}
export interface Archive {
    readonly event_id: string;
    readonly state: 'ARCHIVED';
    readonly revision: number;
    readonly feed: {
        readonly items: readonly ArchivePost[];
        readonly next_cursor: string | null;
    };
    readonly final_mural: FinalMural;
}
type Wire = Archive & {
    final_mural: FinalMural & {
        server_time: string;
        cooldown_ms: number;
        next_allowed_at: null;
    };
};
/** Validate the closed public C04 shape before retaining a deliberately smaller view. */
export function projectArchive(input: unknown, eventId: string): Archive {
    const value = parse<Wire>('Domain_admin_Archive', input);
    if (value.event_id !== eventId || value.final_mural.event_id !== eventId || !ordered(value.feed.items, p => p.post_id)
        || Date.parse(value.final_mural.created_at) > Date.parse(value.final_mural.server_time)
        || value.feed.items.some(p => new Set(p.media_ids).size !== p.media_ids.length))
        throw new Error('Invalid public archive');
    const { snapshot_id, event_id, revision, state, width, height, palette, pixels, created_at } = value.final_mural;
    return freeze({ event_id: value.event_id, state: value.state, revision: value.revision, feed: value.feed,
        final_mural: { snapshot_id, event_id, revision, state, width, height, palette, pixels, created_at } });
}
export const sameMural = (a: FinalMural, b: FinalMural) => a.snapshot_id === b.snapshot_id && a.revision === b.revision && a.created_at === b.created_at
    && a.palette.every((v, i) => v === b.palette[i]) && a.pixels.every((v, i) => v === b.pixels[i]);
