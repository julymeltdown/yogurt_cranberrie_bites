import source from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { validateWholeCharacterConfig } from '../../../../entities/avatar/whole-character'
import { parse, type Page } from '../../contract'
import type { Post } from '../../index'
import { post, id } from '../fixtures'
export { source }
/** Explicit synthetic author-to-config pairs; these are not real people or inferred identity. */
export const rows = parse<Page<Post>>('Domain_feed_FeedPage', { items: source.presets.map((preset, i) => ({
  ...structuredClone(post), post_id: id(900 - i), created_at: '2026-11-04T00:00:00Z',
  author: { ...structuredClone(post.author), user_id: id(700 + i), display_name: `Fixture author ${i + 1}`, avatar: { revision: i + 10, config: validateWholeCharacterConfig(preset.config) } },
  text: `Explicit fixture author ${i + 1} / ${preset.preset_id}`, media_ids: [], links: [{ target_type: 'PARTY', target_id: id(800) }], reaction_count: 42, comment_count: 7,
})), next_cursor: null }).items
