import pack from './character-source-pack.json'

/** User-selected artwork inputs. These source IDs are not runtime AvatarConfig item IDs. */
export const characterSourcePack = pack

/** All selected batches, including additions; archived sources are never selectable or fallback art. */
export const characterArtSources = pack.files

/** A missing source must never fall back to legacy character artwork. */
export function getCharacterSourceUrl(sourceId: string): string {
  const source = pack.files.find(candidate => candidate.source_id === sourceId)
  if (source === undefined) throw new RangeError(`Unknown character source: ${sourceId}`)
  return `${pack.base_url}${source.file}`
}
