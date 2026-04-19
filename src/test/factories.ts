import type { Meal, PersonServing } from '../types/meal'
import type { MealTemplate } from '../types/mealTemplate'
import type { Person } from '../types/person'
import type { Ingredient, Recipe } from '../types/recipe'
import type { AggregatedIngredient, IngredientSource } from '../types/shopping'

export function makePerson(overrides: Partial<Person> = {}): Person {
  return {
    id: 'p1',
    name: 'Alice',
    birthdate: '1990-01-01',
    dietary_goals: null,
    dislikes: '[]',
    favorites: '[]',
    notes: null,
    drink_preferences: null,
    drink_dislikes: null,
    is_active: true,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

export function makeRecipe(overrides: Partial<Recipe> = {}): Recipe {
  const defaultIngredients: Ingredient[] = [
    { name: 'Tomato', amount: { type: 'single', value: 4 }, unit: 'cups' },
  ]
  return {
    id: 'r1',
    name: 'Pasta',
    description: null,
    source: 'manual',
    source_url: null,
    parent_recipe_id: null,
    prep_time: null,
    cook_time: null,
    total_time: null,
    servings: 4,
    portion_size: null,
    instructions: 'Boil water, add pasta.',
    ingredients: JSON.stringify(defaultIngredients),
    nutrition_per_serving: null,
    tags: JSON.stringify([]),
    notes: null,
    icon: null,
    is_favorite: false,
    times_made: 0,
    last_made: null,
    rating: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

export function makeMeal(
  overrides: Partial<Omit<Meal, 'servings'>> & { servings?: PersonServing[] } = {},
): Meal {
  const { servings, ...rest } = overrides
  return {
    id: 'm1',
    date: '2026-04-20',
    meal_type: 'Dinner',
    order_index: 2,
    servings: JSON.stringify(servings ?? []),
    created_at: '2026-04-19T00:00:00Z',
    updated_at: '2026-04-19T00:00:00Z',
    ...rest,
  }
}

export function makeMealTemplate(
  overrides:
    & Partial<Omit<MealTemplate, 'servings'>>
    & { servings?: PersonServing[] } = {},
): MealTemplate {
  const { servings, ...rest } = overrides
  return {
    id: 't1',
    name: 'Family Dinner',
    meal_type: 'Dinner',
    servings: JSON.stringify(servings ?? []),
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
    ...rest,
  }
}

export function makeIngredientSource(
  overrides: Partial<IngredientSource> = {},
): IngredientSource {
  return {
    amount: { type: 'single', value: 4 },
    unit: 'cups',
    source_type: 'recipe',
    source_name: 'Pasta',
    meal_id: 'm1',
    meal_date: '2026-04-20',
    meal_type: 'Dinner',
    recipe_servings: 4,
    person_servings: 4,
    ...overrides,
  }
}

export function makeAggregatedIngredient(
  overrides: Partial<AggregatedIngredient> = {},
): AggregatedIngredient {
  return {
    ingredient_name: 'Tomato',
    total_amount: { type: 'single', value: 4 },
    total_unit: 'cups',
    items: [makeIngredientSource()],
    ...overrides,
  }
}
