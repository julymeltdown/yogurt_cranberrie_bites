import type { ModerationController } from '../../../reports/model/controller'
import { ModerationReview } from '../../../reports/ui/ModerationReview'
import { OperationsSurface } from '../../../parade-ink-operations/OperationsSurface'

export function ContentReviewScreen({ controller }: { controller: ModerationController }) {
  return <OperationsSurface view="content"><ModerationReview controller={controller} /></OperationsSurface>
}
