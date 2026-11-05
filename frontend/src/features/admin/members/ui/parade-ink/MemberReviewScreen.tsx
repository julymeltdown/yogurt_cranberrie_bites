import type { ModerationController } from '../../../reports/model/controller'
import { ModerationReview } from '../../../reports/ui/ModerationReview'
import { OperationsSurface } from '../../../parade-ink-operations/OperationsSurface'

export function MemberReviewScreen({ controller }: { controller: ModerationController }) {
  return <OperationsSurface view="members"><ModerationReview controller={controller} /></OperationsSurface>
}
