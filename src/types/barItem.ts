export type BarItemCategory =
  | 'spirit'
  | 'mixer'
  | 'liqueur'
  | 'bitter'
  | 'juice'
  | 'syrup'
  | 'garnish'
  | 'other'

export interface BarItem {
  id: string
  name: string
  category: BarItemCategory
  created_at: string
}

export interface CreateBarItemDto {
  name: string
  category: BarItemCategory
}

export interface BulkBarItemsDto {
  items: CreateBarItemDto[]
}

export interface BarCategoryMeta {
  value: BarItemCategory
  label: string
  emoji: string
  tagColor: string
}

export const BAR_ITEM_CATEGORIES: BarCategoryMeta[] = [
  {
    value: 'spirit',
    label: 'Spirits',
    emoji: '🥃',
    tagColor: 'bg-amber-50 text-amber-700 border border-amber-200',
  },
  {
    value: 'liqueur',
    label: 'Liqueurs',
    emoji: '🍷',
    tagColor: 'bg-rose-50 text-rose-700 border border-rose-200',
  },
  {
    value: 'mixer',
    label: 'Mixers',
    emoji: '🫧',
    tagColor: 'bg-sky-50 text-sky-700 border border-sky-200',
  },
  {
    value: 'bitter',
    label: 'Bitters',
    emoji: '💧',
    tagColor: 'bg-orange-50 text-orange-700 border border-orange-200',
  },
  {
    value: 'juice',
    label: 'Juices',
    emoji: '🍋',
    tagColor: 'bg-lime-50 text-lime-700 border border-lime-200',
  },
  {
    value: 'syrup',
    label: 'Syrups',
    emoji: '🍯',
    tagColor: 'bg-yellow-50 text-yellow-700 border border-yellow-200',
  },
  {
    value: 'garnish',
    label: 'Garnishes',
    emoji: '🌿',
    tagColor: 'bg-emerald-50 text-emerald-700 border border-emerald-200',
  },
  {
    value: 'other',
    label: 'Other',
    emoji: '✨',
    tagColor: 'bg-violet-50 text-violet-700 border border-violet-200',
  },
]

export const COMMON_BAR_ITEMS: Record<BarItemCategory, string[]> = {
  spirit: [
    'Bourbon',
    'Rye Whiskey',
    'Scotch',
    'Vodka',
    'Gin',
    'White Rum',
    'Dark Rum',
    'Tequila Blanco',
    'Tequila Reposado',
    'Mezcal',
    'Brandy/Cognac',
  ],
  liqueur: [
    'Triple Sec/Cointreau',
    'Kahlúa',
    'Amaretto',
    'Campari',
    'Aperol',
    'St-Germain',
    'Baileys',
    'Chartreuse',
    'Maraschino Liqueur',
    'Crème de Cassis',
  ],
  mixer: ['Tonic Water', 'Club Soda', 'Ginger Beer', 'Ginger Ale', 'Cola', 'Coconut Cream'],
  bitter: ['Angostura Bitters', 'Orange Bitters', "Peychaud's Bitters"],
  juice: [
    'Lime Juice',
    'Lemon Juice',
    'Orange Juice',
    'Cranberry Juice',
    'Pineapple Juice',
    'Grapefruit Juice',
  ],
  syrup: ['Simple Syrup', 'Honey Syrup', 'Grenadine', 'Orgeat', 'Demerara Syrup'],
  garnish: [
    'Lemons',
    'Limes',
    'Oranges',
    'Maraschino Cherries',
    'Olives',
    'Mint',
    'Cocktail Onions',
  ],
  other: ['Coffee', 'Cream/Half-and-Half', 'Egg Whites', 'Tabasco', 'Worcestershire Sauce'],
}
