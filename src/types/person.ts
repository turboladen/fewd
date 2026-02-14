export interface Person {
  id: string
  name: string
  birthdate: string
  dietary_goals: string | null
  dislikes: string
  favorites: string
  notes: string | null
  drink_preferences: string | null
  drink_dislikes: string | null
  is_active: boolean
  created_at: string
  updated_at: string
}

export interface CreatePersonDto {
  name: string
  birthdate: string
  dietary_goals?: string
  dislikes: string[]
  favorites: string[]
  notes?: string
  drink_preferences?: string[]
  drink_dislikes?: string[]
}

export interface UpdatePersonDto {
  name?: string
  birthdate?: string
  dietary_goals?: string
  dislikes?: string[]
  favorites?: string[]
  notes?: string
  is_active?: boolean
  drink_preferences?: string[]
  drink_dislikes?: string[]
}

export function parsePerson(person: Person) {
  return {
    ...person,
    dislikes: JSON.parse(person.dislikes) as string[],
    favorites: JSON.parse(person.favorites) as string[],
    drink_preferences: person.drink_preferences
      ? JSON.parse(person.drink_preferences) as string[]
      : [],
    drink_dislikes: person.drink_dislikes
      ? JSON.parse(person.drink_dislikes) as string[]
      : [],
  }
}
