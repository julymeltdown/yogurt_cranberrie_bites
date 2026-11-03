import { useEffect, useState } from 'react'
import { loadSelectedStaticArtwork, type SelectedStaticRelease } from './release'
import './selected-static.css'

export { validateSelectedStaticRelease } from './release'
export type { SelectedStaticRelease, StaticCharacter } from './release'

/** Decorative selected-source illustrations only; never a fallback for an authenticated avatar. */
export function SelectedStaticGallery({ count = 4, className = '' }: { count?: 1 | 2 | 3 | 4 | 5 | 6; className?: string }) {
  const [image, setImage] = useState<{ url: string; release: SelectedStaticRelease } | null>(null)
  useEffect(() => {
    let stopped = false, url: string | undefined
    void (async () => {
      const { release, bytes } = await loadSelectedStaticArtwork()
      if (stopped) return
      url = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }))
      setImage({ url, release })
    })().catch(() => { /* Missing or unqualified artwork stays absent; never substitute another design. */ })
    return () => { stopped = true; if (url) URL.revokeObjectURL(url) }
  }, [])
  return <div className={`selected-static-gallery ${className}`} data-art-state={image ? 'ready' : 'unavailable'} aria-hidden="true">
    {image?.release.characters.slice(0, count).map((character, index) => <span key={character.id} title={character.label}
      style={{ backgroundImage: `url("${image.url}")`, backgroundPosition: `${index * 20}% 0` }} />)}
  </div>
}
