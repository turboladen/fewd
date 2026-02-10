import { useState } from 'react'
import { useCreateRecipe } from '../hooks/useRecipes'
import { useSetting } from '../hooks/useSettings'
import { useAiSuggestMeals } from '../hooks/useSuggestions'
import type { Person } from '../types/person'
import { parsePerson } from '../types/person'
import type {
  CreateRecipeDto,
  Ingredient,
  IngredientAmount,
  PersonAdaptOptions,
} from '../types/recipe'
import { formatAmount } from '../types/recipe'
import type { MealCharacter } from '../types/suggestion'
import { FieldToggle, PersonSummary } from './PersonFieldToggles'

interface AiSuggestionSectionProps {
  people: Person[]
  selectedPersonIds: Set<string>
  mealType: string
  onApply: (recipeId: string, personIds: string[]) => void
}

interface PersonToggleState {
  include_dietary_goals: boolean
  include_dislikes: boolean
  include_favorites: boolean
}

type Phase = 'configure' | 'results'

const CHARACTER_OPTIONS: { value: MealCharacter['type']; label: string }[] = [
  { value: 'balanced', label: 'Balanced' },
  { value: 'indulgent', label: 'Indulgent' },
  { value: 'quick', label: 'Quick & Easy' },
  { value: 'custom', label: 'Custom' },
]

export function AiSuggestionSection({
  people,
  selectedPersonIds,
  mealType,
  onApply,
}: AiSuggestionSectionProps) {
  const apiKeyQuery = useSetting('anthropic_api_key')
  const aiMutation = useAiSuggestMeals()
  const createRecipeMutation = useCreateRecipe()

  const selectedPeople = people.filter((p) => selectedPersonIds.has(p.id))

  // Per-person field toggles
  const [personToggles, setPersonToggles] = useState<Record<string, PersonToggleState>>(() => {
    const initial: Record<string, PersonToggleState> = {}
    for (const person of people) {
      initial[person.id] = {
        include_dietary_goals: true,
        include_dislikes: true,
        include_favorites: true,
      }
    }
    return initial
  })

  const [characterType, setCharacterType] = useState<MealCharacter['type']>('balanced')
  const [customText, setCustomText] = useState('')
  const [phase, setPhase] = useState<Phase>('configure')
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null)
  const [editingTags, setEditingTags] = useState<Record<number, string[]>>({})
  const [feedback, setFeedback] = useState('')
  const [previousNames, setPreviousNames] = useState<string[]>([])
  const [savingIndex, setSavingIndex] = useState<number | null>(null)

  const hasApiKey = !!apiKeyQuery.data

  const toggleField = (personId: string, field: keyof PersonToggleState) => {
    setPersonToggles((prev) => ({
      ...prev,
      [personId]: { ...prev[personId], [field]: !prev[personId][field] },
    }))
  }

  const buildCharacter = (): MealCharacter => {
    if (characterType === 'custom') {
      return { type: 'custom', text: customText || 'Surprise me' }
    }
    return { type: characterType } as MealCharacter
  }

  const handleGenerate = (prevNames?: string[], userFeedback?: string) => {
    const personOptions: PersonAdaptOptions[] = selectedPeople.map((person) => {
      const toggle = personToggles[person.id] ?? {
        include_dietary_goals: true,
        include_dislikes: true,
        include_favorites: true,
      }
      return {
        person_id: person.id,
        include_dietary_goals: toggle.include_dietary_goals,
        include_dislikes: toggle.include_dislikes,
        include_favorites: toggle.include_favorites,
      }
    })

    setPhase('results')
    aiMutation.mutate(
      {
        person_options: personOptions,
        meal_type: mealType,
        character: buildCharacter(),
        feedback: userFeedback,
        previous_suggestion_names: prevNames,
      },
      {
        onSuccess: (suggestions) => {
          // Initialize tag editing state from returned suggestions
          const tagState: Record<number, string[]> = {}
          for (let i = 0; i < suggestions.length; i++) {
            tagState[i] = [...suggestions[i].tags]
          }
          setEditingTags(tagState)
          setExpandedIndex(null)
        },
      },
    )
  }

  const handleUseThis = (index: number) => {
    const suggestions = aiMutation.data
    if (!suggestions || !suggestions[index]) return

    const dto: CreateRecipeDto = {
      ...suggestions[index],
      tags: editingTags[index] ?? suggestions[index].tags,
    }

    setSavingIndex(index)
    createRecipeMutation.mutate(dto, {
      onSuccess: (recipe) => {
        setSavingIndex(null)
        onApply(recipe.id, [...selectedPersonIds])
      },
      onError: () => {
        setSavingIndex(null)
      },
    })
  }

  const handleRegenerate = () => {
    const currentNames = aiMutation.data?.map((s) => s.name) ?? []
    const allPrevious = [...previousNames, ...currentNames]
    setPreviousNames(allPrevious)
    handleGenerate(allPrevious, feedback || undefined)
    setFeedback('')
  }

  const handleBack = () => {
    setPhase('configure')
    aiMutation.reset()
    setExpandedIndex(null)
    setEditingTags({})
    setFeedback('')
  }

  // No API key
  if (!hasApiKey && !apiKeyQuery.isLoading) {
    return (
      <div className='text-sm text-secondary-600'>
        Set your Anthropic API key in Settings to use AI suggestions.
      </div>
    )
  }

  // Results phase
  if (phase === 'results') {
    return (
      <div className='space-y-2'>
        <div className='flex items-center justify-between'>
          <h5 className='text-sm font-semibold text-secondary-800'>AI Suggestions</h5>
          <button
            onClick={handleBack}
            className='text-xs text-stone-500 hover:text-stone-700'
          >
            {'\u2190'} Back
          </button>
        </div>

        {/* Loading */}
        {aiMutation.isPending && (
          <div className='text-sm text-secondary-600 animate-pulse'>
            Generating suggestions...
          </div>
        )}

        {/* Error */}
        {aiMutation.error && (
          <div className='bg-red-50 border border-red-200 rounded p-2 text-red-700 text-sm'>
            {String(aiMutation.error)}
            <button
              onClick={() => handleGenerate()}
              className='ml-2 text-red-600 underline text-xs'
            >
              Try Again
            </button>
          </div>
        )}

        {/* Suggestion cards */}
        {aiMutation.data && (
          <>
            <div className='space-y-2 max-h-80 overflow-y-auto'>
              {aiMutation.data.map((suggestion, index) => (
                <SuggestionCard
                  key={index}
                  suggestion={suggestion}
                  index={index}
                  isExpanded={expandedIndex === index}
                  onToggle={() => setExpandedIndex(expandedIndex === index ? null : index)}
                  tags={editingTags[index] ?? suggestion.tags}
                  onTagsChange={(tags) => setEditingTags((prev) => ({ ...prev, [index]: tags }))}
                  onUse={() => handleUseThis(index)}
                  isSaving={savingIndex === index}
                />
              ))}
            </div>

            {createRecipeMutation.error && (
              <div className='bg-red-50 border border-red-200 rounded p-2 text-red-700 text-sm'>
                {String(createRecipeMutation.error)}
              </div>
            )}

            {/* Regenerate section */}
            <div className='border-t border-secondary-200 pt-2 space-y-2'>
              <p className='text-xs text-stone-500'>None of these work?</p>
              <textarea
                value={feedback}
                onChange={(e) => setFeedback(e.target.value)}
                placeholder='Tell us what you want instead...'
                className='w-full border border-stone-200 rounded px-2 py-1 text-xs'
                rows={2}
              />
              <button
                onClick={handleRegenerate}
                disabled={aiMutation.isPending}
                className='bg-secondary-600 text-white px-3 py-1 rounded text-xs hover:bg-secondary-700 disabled:opacity-50'
              >
                Regenerate
              </button>
            </div>
          </>
        )}
      </div>
    )
  }

  // Configure phase
  return (
    <div className='space-y-3'>
      <h5 className='text-sm font-semibold text-secondary-800'>AI Suggestions</h5>

      {/* Per-person field toggles */}
      {selectedPeople.length > 0 && (
        <div className='space-y-2'>
          <p className='text-xs text-stone-500'>Include in AI context:</p>
          {selectedPeople.map((person) => {
            const toggle = personToggles[person.id]
            if (!toggle) return null
            const pp = parsePerson(person)

            return (
              <div key={person.id} className='ml-1'>
                <p className='text-xs font-medium text-stone-700 mb-1'>{person.name}</p>
                <div className='flex flex-wrap gap-1 mb-1'>
                  <FieldToggle
                    label='Dietary goals'
                    enabled={toggle.include_dietary_goals}
                    onToggle={() => toggleField(person.id, 'include_dietary_goals')}
                    colorScheme='purple'
                  />
                  <FieldToggle
                    label='Dislikes'
                    enabled={toggle.include_dislikes}
                    onToggle={() => toggleField(person.id, 'include_dislikes')}
                    colorScheme='purple'
                  />
                  <FieldToggle
                    label='Favorites'
                    enabled={toggle.include_favorites}
                    onToggle={() => toggleField(person.id, 'include_favorites')}
                    colorScheme='purple'
                  />
                </div>
                <PersonSummary
                  goals={toggle.include_dietary_goals ? person.dietary_goals : null}
                  dislikes={toggle.include_dislikes ? pp.dislikes : []}
                  favorites={toggle.include_favorites ? pp.favorites : []}
                />
              </div>
            )
          })}
        </div>
      )}

      {/* Meal character */}
      <div>
        <p className='text-xs text-stone-500 mb-1'>Meal character:</p>
        <div className='flex flex-wrap gap-1'>
          {CHARACTER_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              onClick={() => setCharacterType(opt.value)}
              className={`text-xs px-2 py-1 rounded border ${
                characterType === opt.value
                  ? 'bg-secondary-100 border-secondary-400 text-secondary-800'
                  : 'bg-white border-stone-200 text-stone-600 hover:border-secondary-300'
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
        {characterType === 'custom' && (
          <input
            type='text'
            value={customText}
            onChange={(e) => setCustomText(e.target.value)}
            placeholder='e.g., high protein, Mediterranean...'
            className='mt-1 w-full border border-stone-200 rounded px-2 py-1 text-xs'
          />
        )}
      </div>

      {/* Generate button */}
      <button
        onClick={() => handleGenerate()}
        disabled={selectedPeople.length === 0 || aiMutation.isPending}
        className='bg-secondary-600 text-white px-3 py-1 rounded text-xs hover:bg-secondary-700 disabled:opacity-50'
      >
        Generate AI Suggestions
      </button>
    </div>
  )
}

// --- Sub-components ---

function SuggestionCard({
  suggestion,
  index,
  isExpanded,
  onToggle,
  tags,
  onTagsChange,
  onUse,
  isSaving,
}: {
  suggestion: CreateRecipeDto
  index: number
  isExpanded: boolean
  onToggle: () => void
  tags: string[]
  onTagsChange: (tags: string[]) => void
  onUse: () => void
  isSaving: boolean
}) {
  const ingredientPreview = suggestion.ingredients
    .slice(0, 3)
    .map((ing) => ing.name)
    .join(', ')

  return (
    <div className='bg-white border border-stone-200 rounded p-2'>
      {/* Collapsed header */}
      <button
        onClick={onToggle}
        className='w-full text-left'
      >
        <div className='flex items-center gap-2'>
          {suggestion.icon && <span className='text-lg'>{suggestion.icon}</span>}
          <div className='flex-1 min-w-0'>
            <span className='font-medium text-sm'>{suggestion.name}</span>
            {suggestion.description && (
              <p className='text-xs text-stone-500 truncate'>{suggestion.description}</p>
            )}
            <p className='text-xs text-stone-400'>
              {ingredientPreview}
              {suggestion.ingredients.length > 3 ? '...' : ''}
            </p>
          </div>
          <span className='text-xs text-stone-400'>{isExpanded ? '\u25BC' : '\u25B6'}</span>
        </div>
      </button>

      {/* Expanded details */}
      {isExpanded && (
        <div className='mt-2 pt-2 border-t border-stone-100 space-y-2'>
          {/* Ingredients */}
          <div>
            <h6 className='text-xs font-medium text-stone-700 mb-1'>Ingredients</h6>
            <ul className='text-xs space-y-0.5'>
              {suggestion.ingredients.map((ing: Ingredient, i: number) => (
                <li key={i} className='text-stone-600'>
                  <span className='font-medium'>
                    {formatAmount(ing.amount as IngredientAmount)}
                  </span>
                  {ing.unit && ` ${ing.unit}`} {ing.name}
                  {ing.notes && <span className='text-stone-400'>({ing.notes})</span>}
                </li>
              ))}
            </ul>
          </div>

          {/* Instructions (truncated) */}
          <div>
            <h6 className='text-xs font-medium text-stone-700 mb-1'>Instructions</h6>
            <p className='text-xs text-stone-600 whitespace-pre-wrap line-clamp-4'>
              {suggestion.instructions}
            </p>
          </div>

          {/* Nutrition */}
          {suggestion.nutrition_per_serving && (
            <div className='text-xs text-stone-500'>
              <NutritionSummary nutrition={suggestion.nutrition_per_serving} />
            </div>
          )}

          {/* Tags editor */}
          <TagEditor tags={tags} onChange={onTagsChange} />

          {/* Use This button */}
          <button
            onClick={(e) => {
              e.stopPropagation()
              onUse()
            }}
            disabled={isSaving}
            className='bg-secondary-600 text-white px-3 py-1 rounded text-xs hover:bg-secondary-700 disabled:opacity-50'
          >
            {isSaving ? 'Saving...' : `Use This (#${index + 1})`}
          </button>
        </div>
      )}
    </div>
  )
}

function TagEditor({
  tags,
  onChange,
}: {
  tags: string[]
  onChange: (tags: string[]) => void
}) {
  const [input, setInput] = useState('')

  const addTag = () => {
    const tag = input.trim().toLowerCase()
    if (tag && !tags.includes(tag)) {
      onChange([...tags, tag])
    }
    setInput('')
  }

  const removeTag = (index: number) => {
    onChange(tags.filter((_, i) => i !== index))
  }

  return (
    <div>
      <h6 className='text-xs font-medium text-stone-700 mb-1'>Tags</h6>
      <div className='flex flex-wrap gap-1 items-center'>
        {tags.map((tag, i) => (
          <span
            key={i}
            className='bg-secondary-100 text-secondary-700 text-xs px-2 py-0.5 rounded flex items-center gap-1'
          >
            {tag}
            <button
              onClick={() => removeTag(i)}
              className='text-secondary-400 hover:text-secondary-600'
            >
              {'\u00D7'}
            </button>
          </span>
        ))}
        <input
          type='text'
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              addTag()
            }
          }}
          placeholder='Add tag...'
          className='text-xs border border-stone-200 rounded px-2 py-0.5 w-20'
        />
      </div>
    </div>
  )
}

function NutritionSummary({ nutrition }: { nutrition: CreateRecipeDto['nutrition_per_serving'] }) {
  if (!nutrition) return null

  const parts: string[] = []
  if (nutrition.calories != null) parts.push(`${nutrition.calories} cal`)
  if (nutrition.protein_grams != null) parts.push(`${nutrition.protein_grams}g protein`)
  if (nutrition.carbs_grams != null) parts.push(`${nutrition.carbs_grams}g carbs`)
  if (nutrition.fat_grams != null) parts.push(`${nutrition.fat_grams}g fat`)

  if (parts.length === 0) return null

  return <span>{parts.join(' \u2022 ')}</span>
}
