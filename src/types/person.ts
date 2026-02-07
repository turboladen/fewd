export interface Person {
  id: string
  name: string
  birthdate: string
  dietary_goals: string | null
  dislikes: string
  favorites: string
  notes: string | null
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
}

export interface UpdatePersonDto {
  name?: string
  birthdate?: string
  dietary_goals?: string
  dislikes?: string[]
  favorites?: string[]
  notes?: string
  is_active?: boolean
}

export function parsePerson(person: Person) {
  return {
    ...person,
    dislikes: JSON.parse(person.dislikes) as string[],
    favorites: JSON.parse(person.favorites) as string[],
  }
}
