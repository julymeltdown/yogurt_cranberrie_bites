// TEST ONLY. This explicit entry is not imported by the application.
import { createRoot } from 'react-dom/client';
import { ModerationReview } from '../../ui/ModerationReview';
import { harness, id } from './harness';
import type { Lane } from '../../model/contract';
const parameter = new URLSearchParams(location.search).get('lane');
const lane: Lane = parameter === 'members' || parameter === 'content' ? parameter : 'reports';
let mode = 'success';
const fixture = harness(lane, { open: async () => ({ mediaId: id(500), blob: new Blob([Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aWQAAAABJRU5ErkJggg=='), c => c.charCodeAt(0))], { type: 'image/png' }) }) });
fixture.override(request => {
    if (new URL(request.url).pathname.endsWith('/commands'))
        return Response.json(fixture.receipt('FAILED'));
    if (request.method !== 'POST')
        return null;
    if (mode === 'pending')
        return Response.json(fixture.receipt(), { status: 202 });
    if (mode === 'unknown')
        throw new Error('Fixture connection lost after submit');
    if (mode === 'failed')
        return Response.json({ code: 'VALIDATION', message: 'PRIVATE_DIAGNOSTIC', details: { reason: 'VALIDATION_FAILED' } }, { status: 400 });
    return null;
});
if (new URLSearchParams(location.search).has('denied'))
    fixture.setGrant(null);
declare global {
    interface Window {
        __FE37__: {
            mode(value: string): void;
            withdraw(): void;
            writes(): number;
            state(): string;
            receipt(): string | null;
        };
    }
}
window.__FE37__ = { mode(value) {
        mode = value;
    }, withdraw() {
        fixture.setGrant(null);
    }, writes: () => fixture.requests.filter(r => r.method === 'POST').length, state: () => fixture.controller.read().phase, receipt: () => fixture.controller.read().receipt };
createRoot(document.getElementById('root')!).render(<ModerationReview controller={fixture.controller}/>);
