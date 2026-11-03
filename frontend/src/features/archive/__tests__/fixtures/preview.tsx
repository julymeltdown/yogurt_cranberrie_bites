// TEST ONLY: production components with a deterministic public fetch producer.
import { createRoot } from 'react-dom/client';
import { ArchivePage } from '../../ArchivePage';
import { createArchiveController } from '../../controller';
import { createArchiveContext } from '../../context';
import { createPublicArchiveReader } from '../../http';
import { createMemorySession } from '../../../../shared/api/http';
import { page, eventId, id, imageBlob } from './data';
let mode = 'ready';
const requests: Request[] = [], session = createMemorySession(), context = createArchiveContext(eventId, session);
session.replace(0, { userId: id(80), eventId, accessToken: 'TEST_ONLY_PRIVATE_TOKEN' });
const reader = createPublicArchiveReader(async (input) => {
    const request = input as Request;
    requests.push(request);
    if (mode === 'offline')
        throw new Error('PRIVATE_DIAGNOSTIC');
    if (mode === 'unavailable')
        return Response.json({ private_handle: 'PRIVATE_DIAGNOSTIC' }, { status: 404 });
    const result = page(new URL(request.url).searchParams.has('cursor'), new URL(request.url).pathname.split('/')[4]);
    if (mode === 'empty')
        result.feed.items = [];
    if (mode === 'private')
        Object.assign(result.feed.items[0]!, { private_handle: 'PRIVATE_DIAGNOSTIC', state: 'DELETED' });
    return Response.json(result);
});
const controller = createArchiveController({ context, reader, media: { read: async (request) => {
            if (mode === 'image-unavailable')
                throw new Error('gone');
            return { ...request, blob: imageBlob() };
        } } });
declare global {
    interface Window {
        __FE40__: {
            mode(value: string): void;
            event(value: string | null): void;
            account(): void;
            invalidate(): void;
            requests(): {
                method: string;
                credentials: string;
                cache: string;
                authorization: boolean;
                url: string;
            }[];
            state(): string;
        };
    }
}
window.__FE40__ = { mode(value) {
        mode = value;
    }, event(value) {
        context.setEventId(value);
    }, account() {
        session.replace(session.read().generation, { userId: id(81), eventId, accessToken: 'ANOTHER_TEST_ONLY_PRIVATE_TOKEN' });
    }, invalidate() {
        controller.invalidatePost(eventId,id(12));
    }, requests: () => requests.map(r => ({ method: r.method, credentials: r.credentials, cache: r.cache, authorization: r.headers.has('Authorization'), url: r.url })), state: () => JSON.stringify(controller.read()) };
createRoot(document.getElementById('root')!).render(<ArchivePage controller={controller}/>);
