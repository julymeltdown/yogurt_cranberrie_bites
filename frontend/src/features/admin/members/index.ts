import { createModeration, type ModerationOptions } from '../reports/model/controller';
export { ModerationReview as MemberReviewScreen } from '../reports/ui/ModerationReview';
export const createMemberReview = (options: Omit<ModerationOptions, 'lane'>) => createModeration({ ...options, lane: 'members' });
