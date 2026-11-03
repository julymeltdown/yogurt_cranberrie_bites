import { StrictMode, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { setup } from '../../../event-feed/__tests__/fixtures'
import { FeedImage } from '../../../event-feed/FeedImage'
import { createImageUploadController, createMediaUploadTransport } from '../../../media/model/upload-controller'
import { createMediaPreviewController } from '../../../../shared/media/read-media'
import { createFeedThreadController } from '../../controller'
import { FeedThread, PostComposer } from '../../FeedThread'
const fixture = setup((...args) => globalThis.fetch(...args)), auth = fixture.auth
const media = createImageUploadController({ session: auth.session, transport: createMediaUploadTransport(auth.request), purpose: 'POST' })
const thread = createFeedThreadController({ auth, access: fixture.access, media }); thread.start()
const createPreview = (mediaId: string) => createMediaPreviewController({ session: auth.session, request: auth.request, mediaId, purpose: 'POST' })
function Preview() {
  const [page, setPage] = useState<'new' | 'thread'>('new')
  return <><nav><strong>TEST ONLY — actual FE04 controllers / fixture server</strong><button onClick={() => auth.privacy.changePolicy(auth.privacy.capture(), 'block')}>Revoke privacy</button></nav>
    {page === 'new' ? <PostComposer controller={thread} onPost={id => { thread.select(id); setPage('thread') }} /> : <FeedThread controller={thread} onLink={() => undefined} onReport={() => undefined} onEditPost={() => undefined}
      renderMedia={(mediaId, post) => <FeedImage createPreview={createPreview} mediaId={mediaId} eventId={post.event_id} privacy={auth.privacy} alt="Post attachment" />} />}</>
}
createRoot(document.getElementById('root')!).render(<StrictMode><Preview /></StrictMode>)
