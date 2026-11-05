import { createModeration, type ModerationOptions } from '../reports/model/controller';
export { MemberReviewScreen } from './ui/parade-ink/MemberReviewScreen';
export const createMemberReview = (options: Omit<ModerationOptions, 'lane'>) => createModeration({ ...options, lane: 'members' });
