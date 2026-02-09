import { useState } from 'react'
import { useMealSuggestions } from '../hooks/useSuggestions'
import type { Person } from '../types/person'
import type { SuggestionItem } from '../types/suggestion'
import { StarRating } from './StarRating'

function SuggestionSection({
  title,
  items,
  defaultOpen,
  onSelect,
}: {
  title: string
  items: SuggestionItem[]
  defaultOpen: boolean
  onSelect: (recipeId: string) => void
}) {
  const [isOpen, setIsOpen] = useState(defaultOpen)

  return (
    <div>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className='flex items-center gap-1 text-sm font-semibold text-gray-700 w-full text-left py-1'
      >
        <span className='text-xs text-gray-400'>{isOpen ? '\u25BC' : '\u25B6'}</span>
        {title}
        <span className='text-xs text-gray-400 font-normal ml-1'>({items.length})</span>
      </button>
      {isOpen && (
        <div className='space-y-1 ml-4 mb-2'>
          {items.length === 0
            ? <p className='text-xs text-gray-400 italic'>None found</p>
            : items.map((item) => (
              <button
                key={item.recipe_id}
                onClick={() => onSelect(item.recipe_id)}
                className='w-full text-left bg-white border border-gray-200 rounded p-2 hover:border-purple-300 hover:bg-purple-50'
              >
                <div className='flex items-center gap-2'>
                  <span className='font-medium text-sm'>{item.recipe_name}</span>
                  <StarRating value={item.rating} size='sm' />
                </div>
                <div className='text-xs text-gray-500 mt-0.5'>
                  {item.reason}
                </div>
              </button>
            ))}
        </div>
      )}
    </div>
  )
}

export function SuggestionPanel({
  people,
  date,
  onApply,
  onClose,
}: {
  people: Person[]
  date: string
  onApply: (recipeId: string, personIds: string[]) => void
  onClose: () => void
}) {
  const [selectedPersonIds, setSelectedPersonIds] = useState<Set<string>>(
    () => new Set(people.map((p) => p.id)),
  )
  const mutation = useMealSuggestions()

  const togglePerson = (id: string) => {
    setSelectedPersonIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  const handleFetch = () => {
    mutation.mutate({
      person_ids: [...selectedPersonIds],
      reference_date: date,
    })
  }

  const handleSelect = (recipeId: string) => {
    onApply(recipeId, [...selectedPersonIds])
  }

  return (
    <div className='border border-purple-200 rounded-lg p-3 bg-purple-50 mb-3'>
      <div className='flex items-center justify-between mb-2'>
        <h4 className='font-medium text-sm text-purple-800'>Suggest Recipes</h4>
        <button onClick={onClose} className='text-gray-400 hover:text-gray-600 text-sm'>
          {'\u2715'}
        </button>
      </div>

      {/* Person selection */}
      <p className='text-xs text-gray-500 mb-1'>Suggest for:</p>
      <div className='flex flex-wrap gap-2 mb-2'>
        {people.map((person) => {
          const isSelected = selectedPersonIds.has(person.id)
          return (
            <label
              key={person.id}
              className={`flex items-center gap-1.5 text-sm px-2 py-1 rounded border cursor-pointer ${
                isSelected
                  ? 'bg-purple-100 border-purple-400 text-purple-800'
                  : 'bg-white border-gray-200 text-gray-400 line-through'
              }`}
            >
              <input
                type='checkbox'
                checked={isSelected}
                onChange={() => togglePerson(person.id)}
                className='sr-only'
              />
              <span className='text-xs'>
                {isSelected ? '\u2713' : ''}
              </span>
              {person.name}
            </label>
          )
        })}
      </div>

      <button
        onClick={handleFetch}
        disabled={selectedPersonIds.size === 0 || mutation.isPending}
        className='bg-purple-600 text-white px-3 py-1 rounded text-sm hover:bg-purple-700 disabled:opacity-50 mb-2'
      >
        {mutation.isPending ? 'Loading...' : 'Get Suggestions'}
      </button>

      {mutation.error && (
        <div className='mb-2 bg-red-50 border border-red-200 rounded p-2 text-red-700 text-sm'>
          {String(mutation.error)}
        </div>
      )}

      {mutation.data && (
        <div className='space-y-1 max-h-72 overflow-y-auto'>
          <SuggestionSection
            title='Recent Favorites'
            items={mutation.data.recent_favorites}
            defaultOpen={true}
            onSelect={handleSelect}
          />
          <SuggestionSection
            title='Used to Love'
            items={mutation.data.forgotten_hits}
            defaultOpen={true}
            onSelect={handleSelect}
          />
          <SuggestionSection
            title='Something Different'
            items={mutation.data.untried}
            defaultOpen={true}
            onSelect={handleSelect}
          />
        </div>
      )}

      {/* AI placeholder */}
      <div className='mt-2 pt-2 border-t border-purple-200'>
        <p className='text-xs text-gray-400 italic'>
          Want AI suggestions? Coming soon...
        </p>
      </div>
    </div>
  )
}
