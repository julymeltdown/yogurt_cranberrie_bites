import type { ComponentProps } from 'react'
import { WholeCharacterView } from '../../entities/avatar/whole-character'
import { PortraitFrame } from '../../features/avatar/parade-ink/PortraitFrame'

/** Presentation only: the admitted renderer retains its exact inputs and lease. */
export function FramedWholeCharacterView(props: ComponentProps<typeof WholeCharacterView>) {
  const portrait = <WholeCharacterView {...props} />
  const size = props.size ?? 64
  return size === 256 ? portrait : <PortraitFrame label={props.label} size={size}>{portrait}</PortraitFrame>
}
