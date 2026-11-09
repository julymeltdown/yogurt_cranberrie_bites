/**
 * WP-07: the personal room feature.
 *
 * The room is subordinate to the profile. This module owns the room screen, the
 * editor, the poster importer, the pixel conversion and the scoped scene. It
 * deliberately exports nothing that an intro post could embed: there is no room
 * preview component and no room data in any intro post type.
 *
 * The barrel is the feature's *public* surface, not a re-export of every module.
 * A symbol is here only when a consumer outside `features/profile-room` imports
 * it: the screen and its client are imported by the shell panel, and the rest is
 * imported by the feature's own tests. Everything else stays reachable at its
 * module path, so an internal helper does not silently become API.
 */
export { RoomPage, type RoomPageProps } from './RoomPage'
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
export { createPosterConverter, type PosterConverter } from './worker/client'
export { createRoomScene, roomDepth, roomDrawList, CELL_PX, SCENE_PX, type RoomDraw, type RoomScene } from './scene/room-scene'
export {
  GRID_CELLS, initialLayout,
  type CharacterPlacement, type FloorStyle, type FurnitureItem, type FurniturePlacement,
  type PosterPlacement, type RoomLayout, type WallStyle,
} from './model/layout'
export { type Conflict, type EditorAction, type EditorState, type Selection } from './model/editor'
