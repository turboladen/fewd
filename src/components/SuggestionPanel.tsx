import { useState } from 'react'
import { useMealSuggestions } from '../hooks/useSuggestions'
import type { Person } from '../types/person'
import type { SuggestionItem } from '../types/suggestion'
import { AiSuggestionSection } from './AiSuggestionSection'
import { IconCheck, IconChevronDown, IconChevronRight, IconClose } from './Icon'
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
        className='flex items-center gap-1 text-sm font-semibold text-stone-700 w-full text-left py-1'
      >
        <span className='text-xs text-stone-400'>
          {isOpen
            ? <IconChevronDown className='w-3 h-3' />
            : <IconChevronRight className='w-3 h-3' />}
        </span>
        {title}
        <span className='text-xs text-stone-400 font-normal ml-1'>({items.length})</span>
      </button>
      {isOpen && (
        <div className='space-y-1 ml-4 mb-2 animate-slide-down'>
          {items.length === 0
            ? <p className='text-xs text-stone-400 italic'>None found</p>
            : items.map((item) => (
              <button
                key={item.recipe_id}
                onClick={() => onSelect(item.recipe_id)}
                className='w-full text-left card p-2 hover:border-secondary-300 hover:bg-secondary-50'
              >
                <div className='flex items-center gap-2'>
                  <span className='font-medium text-sm'>{item.recipe_name}</span>
                  <StarRating value={item.rating} size='sm' />
                </div>
                <div className='text-xs text-stone-500 mt-0.5'>
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
  mealType,
  onApply,
  onClose,
}: {
  people: Person[]
  date: string
  mealType: string
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
    <div className='border border-secondary-200 rounded-lg p-3 bg-secondary-50 mb-3 animate-slide-down'>
      <div className='flex items-center justify-between mb-2'>
        <h4 className='font-medium text-sm text-secondary-800'>Suggest Recipes</h4>
        <button
          onClick={onClose}
          className='text-stone-400 hover:text-stone-600 text-sm'
          aria-label='Close suggestions'
        >
          <IconClose className='w-3.5 h-3.5' />
        </button>
      </div>

      {/* Person selection */}
      <p className='text-xs text-stone-500 mb-1'>Suggest for:</p>
      <div className='flex flex-wrap gap-2 mb-2'>
        {people.map((person) => {
          const isSelected = selectedPersonIds.has(person.id)
          return (
            <label
              key={person.id}
              className={`flex items-center gap-1.5 text-sm px-2 py-1 rounded border cursor-pointer ${
                isSelected
                  ? 'bg-secondary-100 border-secondary-400 text-secondary-800'
                  : 'bg-white border-stone-200 text-stone-400 line-through'
              }`}
            >
              <input
                type='checkbox'
                checked={isSelected}
                onChange={() => togglePerson(person.id)}
                className='sr-only'
              />
              <span className='text-xs'>
                {isSelected ? <IconCheck className='w-3.5 h-3.5' /> : ''}
              </span>
              {person.name}
            </label>
          )
        })}
      </div>

      <button
        onClick={handleFetch}
        disabled={selectedPersonIds.size === 0 || mutation.isPending}
        className='btn-sm btn-secondary mb-2'
      >
        {mutation.isPending ? 'Loading...' : 'Get Suggestions'}
      </button>

      {mutation.error && (
        <div className='mb-2 panel-error text-red-700 text-sm'>
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

      {/* AI suggestions */}
      <div className='mt-2 pt-2 border-t border-secondary-200'>
        <AiSuggestionSection
          people={people}
          selectedPersonIds={selectedPersonIds}
          mealType={mealType}
          onApply={onApply}
        />
      </div>
    </div>
  )
}
