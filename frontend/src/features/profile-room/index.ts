/**
 * WP-07: the personal room feature.
 *
 * The room is subordinate to the profile. This module owns the room screen, the
 * editor, the poster importer, the pixel conversion and the scoped scene. It
 * deliberately exports nothing that an intro post could embed: there is no room
 * preview component and no room data in any intro post type.
 */
export { RoomPage, type RoomPageProps } from './RoomPage'
export { RoomEditor, type RoomEditorProps } from './RoomEditor'
export { PosterImporter, IMPORT_STEPS, type PosterImporterProps } from './PosterImporter'
export {
  createRoomClient, decodeConflictRoom, decodeOwnRoom, decodeProfileRoom, decodeRoomPoster,
  isRoomUnavailable, ownRoomRequest, profileRoomRequest, saveRoomRequest, setVisibilityRequest,
  uploadPosterRequest, RoomConflictError, RoomUnavailableError, ROOM_PATHS,
  type OwnRoom, type PosterMedia, type ProfileRoom, type RoomClient, type RoomPoster,
  type RoomTransport, type RoomVisibility,
} from './api/room'
export {
  DEFAULT_PRESET, POSTER_PRESETS, PRESET_SIDE, blobToBase64, centredSquare, convertToPoster,
  decodeSource, downsample, encodePng, pixelDigest, validateCrop, PosterConversionError,
  type Crop, type PosterPixels, type PosterPreset,
} from './pixelize'
export { createDirectConverter, createPosterConverter, createWorkerConverter, ConverterUnavailableError, type PosterConverter } from './worker/client'
export { createRoomScene, roomDepth, roomDrawList, CELL_PX, SCENE_PX, type RoomDraw, type RoomScene } from './scene/room-scene'
export {
  FLOOR_STYLES, FURNITURE_ITEMS, GRID_CELLS, MAX_FURNITURE, MAX_POSTERS, WALL_STYLES,
  decodeLayout, initialLayout, layoutKey, sameLayout, validateLayout,
  type CharacterPlacement, type FloorStyle, type FurnitureItem, type FurniturePlacement,
  type PosterPlacement, type RoomLayout, type WallStyle,
} from './model/layout'
export { initialEditorState, isDirty, reduce as reduceEditor, type Conflict, type EditorAction, type EditorState, type Selection } from './model/editor'
