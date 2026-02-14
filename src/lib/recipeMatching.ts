import type { ParsedDrinkRecipe } from '../types/drinkRecipe'

/** Common pantry items that don't require a bar inventory match. */
const PANTRY_ITEMS = [
  'water',
  'ice',
  'sugar',
  'salt',
  'pepper',
  'egg',
  'egg white',
  'egg yolk',
  'cream',
  'milk',
  'butter',
  'honey',
  'cinnamon',
  'nutmeg',
  'vanilla',
  'coffee',
  'tea',
  'cocoa',
  'chocolate',
]

/**
 * Check if an ingredient is covered by a pantry item or any selected bar item.
 * Uses bidirectional case-insensitive substring matching:
 * - "bourbon whiskey" (ingredient) contains "bourbon" (bar item) -> match
 * - "lime juice" (bar item) matches "fresh lime juice" (ingredient) -> match
 */
function ingredientIsCovered(
  ingredientName: string,
  barItemNames: string[],
): boolean {
  const ingLower = ingredientName.toLowerCase().trim()

  if (PANTRY_ITEMS.some((p) => ingLower.includes(p) || p.includes(ingLower))) {
    return true
  }

  return barItemNames.some((barName) => {
    const barLower = barName.toLowerCase().trim()
    return ingLower.includes(barLower) || barLower.includes(ingLower)
  })
}

/**
 * Build a searchable text blob from a recipe's name, description, tags,
 * technique, and ingredient names for style keyword matching.
 */
function recipeSearchText(recipe: ParsedDrinkRecipe): string {
  const parts = [
    recipe.name,
    recipe.description ?? '',
    recipe.technique ?? '',
    ...recipe.tags,
    ...recipe.ingredients.map((ing) => ing.name),
  ]
  return parts.join(' ').toLowerCase()
}

/**
 * Check if a recipe matches the selected style by searching for any
 * of the style's keywords in the recipe's searchable text.
 */
function recipeMatchesStyle(
  searchText: string,
  styleKeywords: string[],
): boolean {
  return styleKeywords.some((kw) => searchText.includes(kw))
}

/**
 * Return saved drink recipes whose ingredients are all covered by the
 * selected bar items, and that match the selected style keywords.
 *
 * If styleKeywords is empty (e.g. custom mood), only ingredient matching is applied.
 * If nonAlcoholicOnly is true, only non-alcoholic recipes are returned.
 */
export function matchRecipesToBarItems(
  recipes: ParsedDrinkRecipe[],
  selectedBarNames: string[],
  styleKeywords: string[] = [],
  nonAlcoholicOnly = false,
): ParsedDrinkRecipe[] {
  if (selectedBarNames.length === 0) return []

  return recipes.filter((recipe) => {
    if (nonAlcoholicOnly && !recipe.is_non_alcoholic) return false

    const ingredientsMatch = recipe.ingredients.every((ing) =>
      ingredientIsCovered(ing.name, selectedBarNames)
    )
    if (!ingredientsMatch) return false

    // If no style keywords, skip style filtering (custom mood or no selection)
    if (styleKeywords.length === 0) return true

    return recipeMatchesStyle(recipeSearchText(recipe), styleKeywords)
  })
}
