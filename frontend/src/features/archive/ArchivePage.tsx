import './parade-ink-private/private.css'
import { useEffect, useRef, useSyncExternalStore } from 'react';
import type { ArchiveController } from './controller';
import type { FinalMural } from './contract';
import type { ArchiveFailure } from './http';
import './archive.css';
const messages: Record<ArchiveFailure, string> = {
    unavailable: 'This archive is unavailable. It may not be published, or the content may have been removed.',
    offline: 'The archive could not be reached. Reconnect and try again.',
    'rate-limited': 'Too many archive requests. Wait before trying again.',
    invalid: 'The public archive could not be verified. No content is displayed.',
    changed: 'The archive changed or content was removed. Reload the current public archive.',
    failed: 'The archive service is unavailable. Try again later.',
};
function FinalWall({ mural }: {
    mural: FinalMural;
}) {
    const canvas = useRef<HTMLCanvasElement>(null);
    useEffect(() => {
        const context = canvas.current?.getContext('2d');
        if (!context)
            return;
        const image = context.createImageData(128, 128);
        const palette = mural.palette.map(hex => [Number.parseInt(hex.slice(1, 3), 16), Number.parseInt(hex.slice(3, 5), 16), Number.parseInt(hex.slice(5, 7), 16)]);
        mural.pixels.forEach((color, index) => {
            const rgb = palette[color]!;
            image.data.set([rgb[0]!, rgb[1]!, rgb[2]!, 255], index * 4);
        });
        context.putImageData(image, 0, 0);
        return () => {
            context.clearRect(0, 0, 128, 128);
        };
    }, [mural]);
    return <figure className="archive-wall"><canvas ref={canvas} width={128} height={128} role="img" aria-label={`Final frozen wall, 128 by 128 pixels, 16 colors, revision ${mural.revision}`}>Final wall: a frozen 128 by 128 pixel mural using 16 colors.</canvas><figcaption>Final wall · revision {mural.revision}. Frozen <time dateTime={mural.created_at}>{mural.created_at}</time></figcaption></figure>;
}
export function ArchivePage({ controller }: {
    controller: ArchiveController;
}) {
    const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), heading = useRef<HTMLHeadingElement>(null);
    useEffect(() => {
        void controller.load();
    }, [controller]);
    useEffect(() => {
        if (state.phase === 'ready')
            heading.current?.focus();
    }, [state.phase, state.page]);
    return <main className="event-archive pi-private-archive"><header><p>Event archive</p><h1>After the show</h1><p>Permitted posts and the final wall. This archive is read-only.</p><p>No live activity, private meeting details or social-sharing records are included.</p><button disabled={state.phase === 'loading'} onClick={() => void controller.load()}>Reload archive</button></header>
    {state.phase === 'idle' && <p role="status">Load the public archive for this event.</p>}
    {state.phase === 'loading' && <p role="status">Reading the public archive…</p>}
    {state.failure && <p role="alert">{messages[state.failure]}</p>}
    {state.data && <><section aria-labelledby="archive-wall-title"><h2 id="archive-wall-title">The final wall</h2><FinalWall mural={state.data.final_mural}/></section>
      <section aria-labelledby="archive-posts-title"><h2 id="archive-posts-title" tabIndex={-1} ref={heading}>Posts after the show</h2><p>Page {state.page} · latest first</p>
        {!state.data.feed.items.length && <p>No permitted posts on this page.</p>}
        <ol className="archive-posts">{state.data.feed.items.map(post => <li key={post.post_id}><article><h3 className="archive-sr">Archived post {post.post_id}</h3><time dateTime={post.created_at}>{post.created_at}</time><p className="archive-copy">{post.text}</p>
          {post.media_ids.map((mediaId, index) => <button key={mediaId} onClick={() => void controller.openMedia(post.post_id, mediaId)} disabled={state.mediaPhase === 'loading'}>View archived image {index + 1}<span className="archive-sr"> for post {post.post_id}</span></button>)}
          {state.media?.postId === post.post_id && <img className="archive-image" src={state.media.url} alt="Image included in this public archived post"/>}
        </article></li>)}</ol>
        {state.mediaPhase === 'loading' && <p role="status">Reading the archived image…</p>}
        {state.mediaPhase === 'unavailable' && <p role="status">Archived image unavailable. No live or private image is substituted.</p>}
        {state.data.feed.next_cursor && <button onClick={() => void controller.load(true)}>Next page</button>}
      </section><footer><p>Archive revision {state.data.revision} · read-only.</p></footer></>}
  </main>;
}
