export { FavoritePicker } from './FavoritePicker'
export { createFavoritesController, type FavoritesController, type FavoritesOptions, type FavoritesState } from './controller'
export {
  addFavorite,
  canAdd,
  CATEGORIES,
  CATEGORY_LABEL,
  CATEGORY_PATH,
  CATEGORY_SELECTION_KEY,
  catalogItemSchema,
  catalogPageSchema,
  decodeCatalogPage,
  emptySelection,
  favoriteItemSchema,
  isSelected,
  itemsFor,
  MAX_PER_CATEGORY,
  removeFavorite,
  selectionSchema,
  validQuery,
  validateSelection,
  type CatalogItem,
  type CatalogPage,
  type Category,
  type FavoriteItem,
  type FavoriteSelection,
} from './contract'
export { catalogExternalLinkLabel, catalogExternalUrl, catalogSubtitle } from './catalog-provider'
