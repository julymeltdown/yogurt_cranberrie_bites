import source from '../../../../../../contracts/fixtures/domain/public-post.json'
import type { Post } from '../../../event-feed/contract'
import type { PrivacyScope } from '../../../../shared/query/privacy'
import type { ProfileActivityInput } from '../../ui/reference-profile/ProfileActivity'
export function activity(scope: PrivacyScope, subject: string): ProfileActivityInput {
  const post = structuredClone(source) as Post
  return { scope, subjectUserId: subject, page: { items: [
    { ...post, event_id: scope.eventId!, author: { ...post.author, user_id: subject }, text: 'See you at the show. Sharing a few photos after the encore.', created_at:'2026-11-04T10:00:00Z', reaction_count:12, comment_count:3 },
    { ...post, post_id:'00000000-0000-4000-8000-00000000003b', event_id:scope.eventId!, author:{...post.author,user_id:subject}, text:'Anyone else making friendship bracelets for the queue?', created_at:'2026-11-04T09:00:00Z', reaction_count:8, comment_count:2 },
  ], next_cursor:'fixture_next_page' } }
}
