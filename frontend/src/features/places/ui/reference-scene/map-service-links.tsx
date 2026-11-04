import { ShellLink } from '../../../../app/navigation'
import { getSpatialV5ServiceAnnotations } from '../../../../world/reference-scene/spatial-v5/map-service'
import type { ReferenceOverviewIdentity } from '../../../../world/reference-scene/metadata'
import { getMap4ServiceAnnotations, type Map4PresentationIdentity } from '../../../../world/reference-scene/map4-service-v1/annotations'
import { map4ServiceTargets } from '../../../../world/reference-scene/map4-service-v1/layout'

/** Virtual feature navigation carries no place ID, membership or physical-arrival authority. */
export function MapServiceLinks({ identity, width, height, onOpen }: {
  identity: ReferenceOverviewIdentity & Map4PresentationIdentity; width: number; height: number; onOpen?(intent: string): void
}) {
  const map4Targets = map4ServiceTargets(identity, width, height)
  return [...getSpatialV5ServiceAnnotations(identity), ...getMap4ServiceAnnotations(identity)].map(annotation => {
    const r = annotation.rect, targetWidth = Math.max(44, r.width / 768 * width)
    const targetHeight = Math.max(44, r.height / 1296 * height)
    const packed = map4Targets.find(target => target.intent === annotation.intent)
    return <ShellLink key={annotation.intent} id={`map-feature-${annotation.intent}`}
      to={annotation.path} onClick={() => onOpen?.(annotation.intent)} className="places-map-plaque" data-map-feature={annotation.intent}
      aria-label={`Open ${annotation.label.toLowerCase()} — virtual feature`} title={`Open ${annotation.label.toLowerCase()}`}
      style={{ left: packed ? packed.left + packed.width / 2 : (r.x + r.width / 2) / 768 * width, top: packed ? packed.top + packed.height / 2 : (r.y + r.height / 2) / 1296 * height,
        width: targetWidth, height: targetHeight, minHeight: 44, maxWidth: 'none', padding: 0,
        borderColor: 'transparent', background: 'transparent', boxShadow: 'none' }}>
      <span style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden', clipPath: 'inset(50%)' }}>{annotation.label}</span>
    </ShellLink>
  })
}
