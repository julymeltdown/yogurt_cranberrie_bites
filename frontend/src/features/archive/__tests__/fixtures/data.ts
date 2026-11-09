export { deferred } from '../../../../test-support/fixtures'
// TEST ONLY: abstract pixel fixture, never production character art or fallback data.
export const id = (n: number) => '00000000-0000-4000-8000-' + String(n).padStart(12, '0');
export const eventId = id(1);
export function page(second = false, event = eventId) {
    return {
        event_id: event, state: 'ARCHIVED', revision: 7,
        feed: { items: [{ post_id: id(second ? 11 : 12), text: second ? 'The last chorus stayed with us.' : 'We sang every word.\nA memory after the show.', media_ids: second ? [] : [id(30)], created_at: second ? '2026-11-07T13:00:00Z' : '2026-11-07T14:00:00Z' }], next_cursor: second ? null : 'page-two' },
        final_mural: { snapshot_id: id(20), event_id: event, revision: 42, state: 'FROZEN', width: 128, height: 128,
            palette: ['#15171C', '#EAE4DA', '#B6494D', '#3C455C', '#6A7E72', '#B09467', '#656A74', '#E0C5A4', '#88424B', '#C9C4B9', '#3F555C', '#738586', '#9C7666', '#493D49', '#969DA3', '#FFF8EB'],
            pixels: Array.from({ length: 16384 }, (_, i) => (Math.floor((i % 128) / 16) + Math.floor(i / 128 / 16)) % 16),
            created_at: '2026-11-07T15:00:00Z', server_time: '2026-11-07T15:01:00Z', cooldown_ms: 5000, next_allowed_at: null }
    };
}
export function imageBlob() {
    return new Blob([Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aWQAAAABJRU5ErkJggg=='), c => c.charCodeAt(0))], { type: 'image/png' });
}
