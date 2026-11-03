import { createModeration, type ModerationOptions } from '../reports/model/controller';
export { ModerationReview as ContentReviewScreen } from '../reports/ui/ModerationReview';
export const createContentReview = (options: Omit<ModerationOptions, 'lane'>) => createModeration({ ...options, lane: 'content' });
