import { useState } from 'react'
import { usePeople } from '../hooks/usePeople'
import { useAdaptRecipe, useCreateRecipe } from '../hooks/useRecipes'
import { useSetting } from '../hooks/useSettings'
import { parsePerson } from '../types/person'
import type { CreateRecipeDto, ParsedRecipe, PersonAdaptOptions } from '../types/recipe'
import { formatAmount } from '../types/recipe'
import { DraftReview } from './DraftReview'
import { IconCheck } from './Icon'
import { FieldToggle, PersonSummary } from './PersonFieldToggles'

interface AdaptRecipePanelProps {
  parsed: ParsedRecipe
  onComplete: (newRecipeId: string) => void
  onEdit: (draft: CreateRecipeDto) => void
  onCancel: () => void
}

interface PersonToggleState {
  selected: boolean
  include_dietary_goals: boolean
  include_dislikes: boolean
  include_favorites: boolean
}

export function AdaptRecipePanel({
  parsed,
  onComplete,
  onEdit,
  onCancel,
}: AdaptRecipePanelProps) {
  const { data: people } = usePeople()
  const apiKeyQuery = useSetting('anthropic_api_key')
  const adaptMutation = useAdaptRecipe()
  const createMutation = useCreateRecipe()

  const activePeople = people?.filter((p) => p.is_active) ?? []

  // Per-person toggle state
  const [personToggles, setPersonToggles] = useState<Record<string, PersonToggleState>>(() => {
    const initial: Record<string, PersonToggleState> = {}
    // Will be populated once people load
    return initial
  })

  // Ensure toggles exist for all active people
  if (activePeople.length > 0 && Object.keys(personToggles).length === 0) {
    const initial: Record<string, PersonToggleState> = {}
    for (const person of activePeople) {
      initial[person.id] = {
        selected: true,
        include_dietary_goals: true,
        include_dislikes: true,
        include_favorites: true,
      }
    }
    setPersonToggles(initial)
  }

  const [instructions, setInstructions] = useState('')
  const [draft, setDraft] = useState<CreateRecipeDto | null>(null)
  const [phase, setPhase] = useState<'configure' | 'review'>('configure')

  const hasApiKey = !!apiKeyQuery.data

  const selectedCount = Object.values(personToggles).filter((t) => t.selected).length

  const togglePerson = (personId: string) => {
    setPersonToggles((prev) => ({
      ...prev,
      [personId]: { ...prev[personId], selected: !prev[personId].selected },
    }))
  }

  const toggleField = (personId: string, field: keyof Omit<PersonToggleState, 'selected'>) => {
    setPersonToggles((prev) => ({
      ...prev,
      [personId]: { ...prev[personId], [field]: !prev[personId][field] },
    }))
  }

  const handleGenerate = () => {
    const personOptions: PersonAdaptOptions[] = Object.entries(personToggles)
      .filter(([, t]) => t.selected)
      .map(([personId, t]) => ({
        person_id: personId,
        include_dietary_goals: t.include_dietary_goals,
        include_dislikes: t.include_dislikes,
        include_favorites: t.include_favorites,
      }))

    setPhase('review')
    adaptMutation.mutate(
      {
        recipe_id: parsed.id,
        person_options: personOptions,
        user_instructions: instructions,
      },
      {
        onSuccess: (result) => {
          setDraft(result)
        },
      },
    )
  }

  const handleAccept = () => {
    if (!draft) return
    createMutation.mutate(draft, {
      onSuccess: (recipe) => {
        onComplete(recipe.id)
      },
    })
  }

  const handleEditDraft = () => {
    if (!draft) return
    onEdit(draft)
  }

  const handleReject = () => {
    setDraft(null)
    adaptMutation.reset()
    setPhase('configure')
  }

  const handleRegenerate = () => {
    setDraft(null)
    handleGenerate()
  }

  // No API key message
  if (!hasApiKey && !apiKeyQuery.isLoading) {
    return (
      <div className='panel-secondary'>
        <h3 className='font-semibold text-lg mb-3 text-secondary-900'>
          Adapt: {parsed.name}
        </h3>
        <div className='text-sm text-secondary-700 mb-3'>
          Set your Anthropic API key in Settings to use AI features.
        </div>
        <button
          onClick={onCancel}
          className='btn-sm btn-outline'
        >
          Cancel
        </button>
      </div>
    )
  }

  // Review phase
  if (phase === 'review') {
    return (
      <div className='panel-secondary animate-fade-in'>
        <h3 className='font-semibold text-lg mb-3 text-secondary-900'>
          Adapt: {parsed.name}
        </h3>
        <DraftReview
          isLoading={adaptMutation.isPending}
          error={adaptMutation.error ? String(adaptMutation.error) : null}
          onAccept={handleAccept}
          onEdit={handleEditDraft}
          onReject={handleReject}
          onRegenerate={handleRegenerate}
          onCancel={handleReject}
          acceptLabel={createMutation.isPending ? 'Saving...' : 'Save Recipe'}
          editLabel='Edit First'
        >
          {draft && <RecipePreview draft={draft} />}
        </DraftReview>
        {createMutation.error && (
          <div className='mt-2 panel-error text-red-700 text-sm'>
            {String(createMutation.error)}
          </div>
        )}
      </div>
    )
  }

  // Configure phase
  return (
    <div className='panel-secondary animate-slide-up'>
      <h3 className='font-semibold text-lg mb-3 text-secondary-900'>
        Adapt: {parsed.name}
      </h3>

      {/* Person selection */}
      <div className='mb-4'>
        <p className='text-sm font-medium text-stone-700 mb-2'>Adapt for:</p>
        <div className='space-y-2'>
          {activePeople.map((person) => {
            const toggle = personToggles[person.id]
            if (!toggle) return null
            const pp = parsePerson(person)

            return (
              <div key={person.id}>
                <label
                  className={`flex items-center gap-2 text-sm px-3 py-2 rounded border cursor-pointer ${
                    toggle.selected
                      ? 'bg-secondary-100 border-secondary-400'
                      : 'bg-white border-stone-200'
                  }`}
                >
                  <input
                    type='checkbox'
                    checked={toggle.selected}
                    onChange={() => togglePerson(person.id)}
                    className='sr-only'
                  />
                  <span className='text-secondary-700'>
                    {toggle.selected ? <IconCheck className='w-3.5 h-3.5' /> : ''}
                  </span>
                  <span className='font-medium'>{person.name}</span>
                </label>

                {/* Field toggles + profile summary */}
                {toggle.selected && (
                  <div className='ml-6 mt-1 mb-2 space-y-1'>
                    <div className='flex flex-wrap gap-2'>
                      <FieldToggle
                        label='Dietary goals'
                        enabled={toggle.include_dietary_goals}
                        onToggle={() => toggleField(person.id, 'include_dietary_goals')}
                      />
                      <FieldToggle
                        label='Dislikes'
                        enabled={toggle.include_dislikes}
                        onToggle={() => toggleField(person.id, 'include_dislikes')}
                      />
                      <FieldToggle
                        label='Favorites'
                        enabled={toggle.include_favorites}
                        onToggle={() => toggleField(person.id, 'include_favorites')}
                      />
                    </div>
                    <PersonSummary
                      goals={toggle.include_dietary_goals ? person.dietary_goals : null}
                      dislikes={toggle.include_dislikes ? pp.dislikes : []}
                      favorites={toggle.include_favorites ? pp.favorites : []}
                    />
                  </div>
                )}
              </div>
            )
          })}
        </div>
      </div>

      {/* Instructions */}
      <div className='mb-4'>
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          Additional instructions
        </label>
        <textarea
          value={instructions}
          onChange={(e) => setInstructions(e.target.value)}
          placeholder='e.g., Make this keto-friendly, use chicken thighs instead...'
          className='input w-full'
          rows={3}
        />
      </div>

      {/* Actions */}
      <div className='flex gap-2'>
        <button
          onClick={handleGenerate}
          disabled={selectedCount === 0}
          className='btn-sm btn-secondary'
        >
          Generate Adapted Recipe
        </button>
        <button
          onClick={onCancel}
          className='btn-sm btn-outline'
        >
          Cancel
        </button>
      </div>
    </div>
  )
}

// --- Sub-components ---

function RecipePreview({ draft }: { draft: CreateRecipeDto }) {
  return (
    <div className='space-y-2'>
      <div className='flex items-center gap-2'>
        {draft.icon && <span className='text-xl'>{draft.icon}</span>}
        <h4 className='font-semibold text-lg'>{draft.name}</h4>
      </div>

      {draft.description && <p className='text-sm text-stone-600'>{draft.description}</p>}

      <p className='text-sm text-stone-500'>Serves {draft.servings}</p>

      {/* Ingredients */}
      <div>
        <h5 className='text-sm font-medium mb-1'>Ingredients</h5>
        <ul className='text-sm space-y-0.5'>
          {draft.ingredients.map((ing, i) => (
            <li key={i} className='text-stone-700'>
              <span className='font-medium'>{formatAmount(ing.amount)}</span>
              {ing.unit && ` ${ing.unit}`} {ing.name}
              {ing.notes && <span className='text-stone-400'>({ing.notes})</span>}
            </li>
          ))}
        </ul>
      </div>

      {/* Instructions (truncated) */}
      <div>
        <h5 className='text-sm font-medium mb-1'>Instructions</h5>
        <p className='text-sm text-stone-700 whitespace-pre-wrap line-clamp-6'>
          {draft.instructions}
        </p>
      </div>

      {/* Tags */}
      {draft.tags.length > 0 && (
        <div className='flex flex-wrap gap-1'>
          {draft.tags.map((tag, i) => (
            <span key={i} className='tag'>
              {tag}
            </span>
          ))}
        </div>
      )}

      {/* Adaptation notes */}
      {draft.notes && (
        <div className='text-xs text-stone-500 italic border-t border-stone-200 pt-2 mt-2'>
          {draft.notes}
        </div>
      )}
    </div>
  )
}
