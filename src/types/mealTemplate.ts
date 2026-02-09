import type { PersonServing } from './meal'

export interface MealTemplate {
  id: string
  name: string
  meal_type: string
  servings: string // JSON string of PersonServing[]
  created_at: string
  updated_at: string
}

export interface ParsedMealTemplate extends Omit<MealTemplate, 'servings'> {
  servings: PersonServing[]
}

export interface CreateMealTemplateDto {
  name: string
  meal_type: string
  servings: PersonServing[]
}

export interface UpdateMealTemplateDto {
  name?: string
  meal_type?: string
  servings?: PersonServing[]
}

export interface CreateFromMealDto {
  meal_id: string
  name: string
}

export function parseMealTemplate(template: MealTemplate): ParsedMealTemplate {
  return {
    ...template,
    servings: JSON.parse(template.servings) as PersonServing[],
  }
}
