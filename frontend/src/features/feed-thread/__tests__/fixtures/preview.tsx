import { createRoot } from 'react-dom/client'
import { useState } from 'react'
import { setup, post } from '../../../event-feed/__tests__/fixtures'
import { EventFeed } from '../../../event-feed/EventFeed'
import { createFeedThreadController } from '../../controller'
import { FeedThread, PostComposer } from '../../FeedThread'
const fixture = setup((...args) => globalThis.fetch(...args)), feed = fixture.controller, thread = createFeedThreadController({ auth: fixture.auth, access: fixture.access })
feed.start(); thread.start()
function Preview() {
  const [page, setPage] = useState<'feed' | 'new' | 'thread' | 'edit'>('feed')
  const open = (id: string) => { if (thread.select(id)) setPage('thread') }
  return <><nav aria-label="Test fixture controls"><strong>TEST ONLY — native HTTP fixtures</strong><button onClick={() => { setPage('feed'); void feed.loadFeed() }}>Feed</button><button onClick={() => { fixture.auth.privacy.changePolicy(fixture.auth.privacy.capture(), 'block') }}>Revoke privacy</button><button onClick={() => { fixture.setAccess({ available: true, readOnly: true }) }}>Read-only event</button></nav>
    {page === 'feed' ? <EventFeed controller={feed} onPost={open} onCompose={() => { if (thread.select(null)) setPage('new') }} onLink={() => { feed.removeLink(post.links[0]!) }} /> : page === 'new' || page === 'edit' ? <PostComposer controller={thread} onPost={open} /> : <FeedThread controller={thread} onLink={() => undefined} onEditPost={() => setPage('edit')} onReport={() => undefined} />}</>
}
createRoot(document.getElementById('root')!).render(<Preview />)
