import { createModeration, type ModerationOptions } from '../reports/model/controller';
export { ContentReviewScreen } from './ui/parade-ink/ContentReviewScreen';
export const createContentReview = (options: Omit<ModerationOptions, 'lane'>) => createModeration({ ...options, lane: 'content' });
