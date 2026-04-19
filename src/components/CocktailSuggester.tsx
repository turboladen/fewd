import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { useBarItems } from '../hooks/useBarItems'
import { useAiSuggestCocktails } from '../hooks/useCocktailSuggestions'
import { useCreateDrinkRecipe, useDrinkRecipes } from '../hooks/useDrinkRecipes'
import { usePeople } from '../hooks/usePeople'
import { useAvailableModels, useSetSetting, useSetting } from '../hooks/useSettings'
import { matchRecipesToBarItems } from '../lib/recipeMatching'
import type { BarItem } from '../types/barItem'
import { BAR_ITEM_CATEGORIES } from '../types/barItem'
import type {
  CreateDrinkRecipeDto,
  DrinkMood,
  ParsedDrinkRecipe,
  SuggestionSource,
} from '../types/drinkRecipe'
import { parseDrinkRecipe } from '../types/drinkRecipe'
import { parsePerson } from '../types/person'
import { formatAmount } from '../types/recipe'
import { IconArrowLeft, IconChevronDown, IconChevronRight, IconRefresh, IconWarning } from './Icon'
import { StarRating } from './StarRating'
import { useToast } from './Toast'

type Phase = 'configure' | 'results'

interface StyleOption {
  label: string
  emoji: string
  description: string
  /** Keywords matched against recipe name, description, tags, technique, and ingredients for recipe-book filtering. */
  keywords: string[]
}

const STYLE_OPTIONS: StyleOption[] = [
  {
    label: 'Ancestrals',
    emoji: '🥃',
    description: 'Old Fashioned, Sazerac',
    keywords: ['old fashioned', 'sazerac', 'ancestral', 'spirit-forward'],
  },
  {
    label: 'Sours',
    emoji: '🍋',
    description: 'Whiskey Sour, Daiquiri, Margarita',
    keywords: ['sour', 'daiquiri', 'margarita', 'gimlet', 'shaken', 'lemon juice', 'lime juice'],
  },
  {
    label: 'Spirit-Forward',
    emoji: '🍸',
    description: 'Martini, Manhattan, Negroni',
    keywords: ['martini', 'manhattan', 'negroni', 'boulevardier', 'spirit-forward', 'vermouth'],
  },
  {
    label: 'Duos & Trios',
    emoji: '🎭',
    description: 'Simple spirit-and-modifier combos',
    keywords: ['duo', 'trio', 'two-ingredient', 'simple'],
  },
  {
    label: 'Digestifs',
    emoji: '🫗',
    description: 'After-dinner, bitter, amaro-based',
    keywords: ['digestif', 'amaro', 'after-dinner', 'fernet'],
  },
  {
    label: 'Apéritifs',
    emoji: '🥂',
    description: 'Light, pre-dinner, appetite-opening',
    keywords: ['aperitif', 'aperol', 'pre-dinner', 'spritz'],
  },
  {
    label: 'Champagne',
    emoji: '🍾',
    description: 'French 75, Bellini, Kir Royale',
    keywords: ['champagne', 'french 75', 'bellini', 'kir', 'prosecco', 'sparkling', 'mimosa'],
  },
  {
    label: 'Highballs & Fizzes',
    emoji: '🫧',
    description: 'Collins, Rickey, Gin Fizz',
    keywords: ['highball', 'fizz', 'collins', 'rickey', 'buck', 'tonic'],
  },
  {
    label: 'Juleps & Smashes',
    emoji: '🌿',
    description: 'Muddled herbs, refreshing',
    keywords: ['julep', 'smash', 'muddled', 'mint julep'],
  },
  {
    label: 'Hot Drinks',
    emoji: '☕',
    description: 'Toddy, Irish Coffee, mulled',
    keywords: ['hot', 'toddy', 'irish coffee', 'mulled', 'warm', 'heated'],
  },
  {
    label: 'Flips & Nogs',
    emoji: '🥚',
    description: 'Egg-based, creamy, rich',
    keywords: ['flip', 'nog', 'eggnog', 'egg white', 'egg yolk'],
  },
  {
    label: 'Pousse Family',
    emoji: '🌈',
    description: 'Layered, Pousse-Café style',
    keywords: ['pousse', 'layered', 'float'],
  },
  {
    label: 'Tropical',
    emoji: '🌺',
    description: 'Tiki, Mai Tai, Piña Colada',
    keywords: ['tropical', 'tiki', 'mai tai', 'colada', 'zombie', 'coconut', 'pineapple'],
  },
  {
    label: 'Punch',
    emoji: '🍹',
    description: 'Batch, communal, party-style',
    keywords: ['punch', 'batch', 'communal', 'party', 'bowl'],
  },
  {
    label: 'Old & Odd Birds',
    emoji: '🦜',
    description: 'Unusual, vintage, forgotten',
    keywords: ['vintage', 'forgotten', 'unusual', 'obscure', 'pre-prohibition'],
  },
  {
    label: 'Refreshing',
    emoji: '❄️',
    description: 'Light, crisp, easy-drinking',
    keywords: ['refreshing', 'light', 'crisp', 'cooler', 'spritzer'],
  },
]

/** Compute someone's age from a birthdate string. */
function computeAge(birthdate: string): number {
  return Math.floor(
    (Date.now() - new Date(birthdate).getTime()) / (365.25 * 24 * 60 * 60 * 1000),
  )
}

export function CocktailSuggester() {
  const { data: barItems, isLoading: barLoading } = useBarItems()
  const { data: people, isLoading: peopleLoading } = usePeople()
  const apiKeyQuery = useSetting('anthropic_api_key')
  const modelQuery = useSetting('claude_model')
  const modelsQuery = useAvailableModels()
  const setSetting = useSetSetting()
  const aiMutation = useAiSuggestCocktails()
  const createDrinkMutation = useCreateDrinkRecipe()
  const { data: drinkRecipes } = useDrinkRecipes()
  const { toast } = useToast()

  // Selection state
  const [selectedBarIds, setSelectedBarIds] = useState<Set<string>>(new Set())
  const [selectedPersonIds, setSelectedPersonIds] = useState<Set<string>>(new Set())
  const [barInitialized, setBarInitialized] = useState(false)
  const [peopleInitialized, setPeopleInitialized] = useState(false)

  // Initialize selections when data loads
  useEffect(() => {
    if (barItems && !barInitialized) {
      setSelectedBarIds(new Set(barItems.map((i) => i.id)))
      setBarInitialized(true)
    }
  }, [barItems, barInitialized])

  useEffect(() => {
    if (people && !peopleInitialized) {
      const ofAge = people.filter((p) => computeAge(p.birthdate) >= 21)
      setSelectedPersonIds(new Set(ofAge.map((p) => p.id)))
      setPeopleInitialized(true)
    }
  }, [people, peopleInitialized])

  const [selectedStyles, setSelectedStyles] = useState<Set<string>>(new Set(['Ancestrals']))
  const [customMoodText, setCustomMoodText] = useState('')
  const [isCustomMood, setIsCustomMood] = useState(false)
  const [phase, setPhase] = useState<Phase>('configure')
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null)
  const [feedback, setFeedback] = useState('')
  const [previousNames, setPreviousNames] = useState<string[]>([])
  const [savingIndex, setSavingIndex] = useState<number | null>(null)
  const [savedIndices, setSavedIndices] = useState<Set<number>>(new Set())
  const [showIngredients, setShowIngredients] = useState(false)
  const [suggestionSource, setSuggestionSource] = useState<SuggestionSource>('both')
  const [matchedRecipes, setMatchedRecipes] = useState<ParsedDrinkRecipe[]>([])
  const [expandedRecipeId, setExpandedRecipeId] = useState<string | null>(null)

  const hasApiKey = !!apiKeyQuery.data

  const toggleBarItem = (id: string) => {
    setSelectedBarIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const toggleAllBarCategory = (items: BarItem[]) => {
    const categoryIds = items.map((i) => i.id)
    const allSelected = categoryIds.every((id) => selectedBarIds.has(id))
    setSelectedBarIds((prev) => {
      const next = new Set(prev)
      for (const id of categoryIds) {
        if (allSelected) next.delete(id)
        else next.add(id)
      }
      return next
    })
  }

  const togglePerson = (id: string) => {
    setSelectedPersonIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  // Check if any selected person is under 21
  const hasMinor = people?.some((p) => selectedPersonIds.has(p.id) && computeAge(p.birthdate) < 21)
    ?? false

  const toggleStyle = (label: string) => {
    setSelectedStyles((prev) => {
      const next = new Set(prev)
      if (next.has(label)) {
        // Prevent deselecting the last style — always keep at least one
        if (next.size <= 1) return prev
        next.delete(label)
      } else {
        next.add(label)
      }
      return next
    })
    setIsCustomMood(false)
  }

  const rollDice = () => {
    const shuffled = [...STYLE_OPTIONS].sort(() => Math.random() - 0.5)
    const count = 2 + Math.floor(Math.random() * 2) // 2-3
    setSelectedStyles(new Set(shuffled.slice(0, count).map((o) => o.label)))
    setIsCustomMood(false)
  }

  const buildMood = (): DrinkMood => {
    if (isCustomMood) {
      return { type: 'custom', text: customMoodText || 'Surprise me' }
    }
    const labels = Array.from(selectedStyles)
    return { type: 'style', label: labels.length > 0 ? labels.join(', ') : 'Ancestrals' }
  }

  const computeRecipeMatches = (): ParsedDrinkRecipe[] => {
    if (!drinkRecipes || !barItems) return []
    const selectedNames = barItems
      .filter((item) => selectedBarIds.has(item.id))
      .map((item) => item.name)
    const parsed = drinkRecipes.map(parseDrinkRecipe)
    // Gather keywords from all selected styles; custom mood passes empty (no style filter)
    const keywords = isCustomMood
      ? []
      : STYLE_OPTIONS.filter((o) => selectedStyles.has(o.label)).flatMap((o) => o.keywords)
    return matchRecipesToBarItems(parsed, selectedNames, keywords, hasMinor)
  }

  const handleGenerate = (prevNames?: string[], userFeedback?: string) => {
    // Compute recipe matches for 'both' and 'recipes-only'
    if (suggestionSource !== 'ai-only') {
      setMatchedRecipes(computeRecipeMatches())
    } else {
      setMatchedRecipes([])
    }

    // Transition to results immediately — AI loads asynchronously in the results view
    setPhase('results')
    setExpandedIndex(null)
    setExpandedRecipeId(null)
    setSavedIndices(new Set())
    setFeedback('')

    // Call AI for 'both' and 'ai-only'
    if (suggestionSource !== 'recipes-only') {
      aiMutation.mutate({
        person_ids: Array.from(selectedPersonIds),
        bar_item_ids: Array.from(selectedBarIds),
        mood: buildMood(),
        include_non_alcoholic: hasMinor,
        feedback: userFeedback,
        previous_suggestion_names: prevNames,
      })
    }
  }

  const handleRegenerate = () => {
    const names = [
      ...previousNames,
      ...(aiMutation.data?.map((s) => s.name) ?? []),
    ]
    setPreviousNames(names)
    handleGenerate(names, feedback || undefined)
  }

  const handleSave = (suggestion: CreateDrinkRecipeDto, index: number) => {
    setSavingIndex(index)
    createDrinkMutation.mutate(suggestion, {
      onSuccess: () => {
        setSavingIndex(null)
        setSavedIndices((prev) => new Set(prev).add(index))
        toast(`Saved "${suggestion.name}"`)
      },
      onError: () => {
        setSavingIndex(null)
      },
    })
  }

  // Group bar items by category
  const groupedBar = (barItems ?? []).reduce<Record<string, BarItem[]>>((acc, item) => {
    if (!acc[item.category]) acc[item.category] = []
    acc[item.category].push(item)
    return acc
  }, {})

  const isLoading = barLoading || peopleLoading

  if (isLoading) {
    return <div className='p-6 text-stone-500 animate-pulse'>Loading...</div>
  }

  // ─── Results Phase ─────────────────────────────────────────────

  if (phase === 'results') {
    const hasAiResults = suggestionSource !== 'recipes-only' && aiMutation.data
      && aiMutation.data.length > 0
    const hasRecipeResults = suggestionSource !== 'ai-only' && matchedRecipes.length > 0
    const showingBoth = suggestionSource === 'both'

    return (
      <div className='p-6 space-y-4'>
        <button
          onClick={() => setPhase('configure')}
          className='btn-sm btn-ghost text-stone-500'
        >
          <IconArrowLeft className='w-4 h-4' />
          Back to setup
        </button>

        {/* Input summary */}
        <div className='text-xs text-stone-400 flex flex-wrap items-center gap-x-2 gap-y-0.5'>
          <span className='font-medium text-stone-500'>
            {suggestionSource === 'recipes-only' ? 'Matched for:' : 'Generated for:'}
          </span>
          <span>
            {people
              ?.filter((p) => selectedPersonIds.has(p.id))
              .map((p) => p.name)
              .join(', ') || 'no one'}
          </span>
          <span>·</span>
          <span>
            {isCustomMood
              ? `"${customMoodText || 'Surprise me'}"`
              : selectedStyles.size > 0
              ? Array.from(selectedStyles).join(', ')
              : 'any style'}
          </span>
          <span>·</span>
          <span>{selectedBarIds.size} ingredient{selectedBarIds.size !== 1 ? 's' : ''}</span>
          {hasMinor && (
            <>
              <span>·</span>
              <span>incl. non-alcoholic</span>
            </>
          )}
        </div>

        {/* ── Recipe Book Matches ── */}
        {suggestionSource !== 'ai-only' && (
          <div>
            {showingBoth && (
              <h2 className='text-sm font-semibold text-stone-700 uppercase tracking-wide mb-3'>
                From Your Recipe Book
              </h2>
            )}
            {hasRecipeResults
              ? (
                <div className='space-y-3'>
                  {matchedRecipes.map((recipe) => (
                    <RecipeMatchCard
                      key={recipe.id}
                      recipe={recipe}
                      isExpanded={expandedRecipeId === recipe.id}
                      onToggle={() =>
                        setExpandedRecipeId(
                          expandedRecipeId === recipe.id ? null : recipe.id,
                        )}
                    />
                  ))}
                </div>
              )
              : (
                <div className='card p-4 text-sm text-stone-500 text-center'>
                  No saved recipes match your selected ingredients.
                </div>
              )}
          </div>
        )}

        {/* ── AI Suggestions ── */}
        {suggestionSource !== 'recipes-only' && (
          <div>
            {showingBoth && (
              <h2 className='text-sm font-semibold text-stone-700 uppercase tracking-wide mb-3'>
                AI Suggestions
              </h2>
            )}
            {aiMutation.isPending && (
              <div className='card p-4 text-stone-500 animate-pulse text-center'>
                {aiMutation.progress?.message ?? 'Preparing...'}
                {aiMutation.progress?.phase === 'generating' && aiMutation.progress.tokens
                  ? ` (${aiMutation.progress.tokens} tokens)`
                  : ''}
              </div>
            )}
            {aiMutation.error && (
              <div className='panel-error text-sm text-red-700'>
                {aiMutation.error instanceof Error
                  ? aiMutation.error.message
                  : 'Something went wrong'}
              </div>
            )}
            {hasAiResults && (
              <div className='space-y-3'>
                {aiMutation.data!.map((suggestion, i) => {
                  const isExpanded = expandedIndex === i
                  return (
                    <div key={i} className='card p-4 animate-slide-up'>
                      <button
                        onClick={() => setExpandedIndex(isExpanded ? null : i)}
                        className='flex items-center gap-3 w-full text-left'
                      >
                        <span className='text-2xl flex-shrink-0'>
                          {suggestion.icon || '🍸'}
                        </span>
                        <div className='flex-1 min-w-0'>
                          <h3 className='font-semibold text-stone-900'>{suggestion.name}</h3>
                          {suggestion.description && (
                            <p className='text-sm text-stone-500 line-clamp-2'>
                              {suggestion.description}
                            </p>
                          )}
                          <div className='flex flex-wrap gap-1.5 mt-1'>
                            {suggestion.technique && (
                              <span className='tag text-xs'>{suggestion.technique}</span>
                            )}
                            {suggestion.glassware && (
                              <span className='tag text-xs'>{suggestion.glassware}</span>
                            )}
                            {suggestion.is_non_alcoholic && (
                              <span className='tag text-xs bg-green-100 text-green-700'>
                                non-alcoholic
                              </span>
                            )}
                          </div>
                        </div>
                        {isExpanded
                          ? <IconChevronDown className='w-4 h-4 text-stone-400' />
                          : <IconChevronRight className='w-4 h-4 text-stone-400' />}
                      </button>

                      {isExpanded && (
                        <div className='mt-4 pt-3 border-t border-stone-200 space-y-3 animate-fade-in'>
                          <div>
                            <h4 className='text-sm font-semibold text-stone-700 mb-1'>
                              Ingredients
                            </h4>
                            <ul className='text-sm text-stone-600 space-y-0.5'>
                              {suggestion.ingredients.map((ing, j) => (
                                <li key={j}>
                                  {formatAmount(ing.amount)} {ing.unit} {ing.name}
                                  {ing.notes && (
                                    <span className='text-stone-400'>({ing.notes})</span>
                                  )}
                                </li>
                              ))}
                            </ul>
                          </div>
                          <div>
                            <h4 className='text-sm font-semibold text-stone-700 mb-1'>
                              Instructions
                            </h4>
                            <p className='text-sm text-stone-600 whitespace-pre-line'>
                              {suggestion.instructions}
                            </p>
                          </div>
                          {suggestion.garnish && (
                            <p className='text-sm'>
                              <span className='font-semibold text-stone-700'>Garnish:</span>{' '}
                              <span className='text-stone-600'>{suggestion.garnish}</span>
                            </p>
                          )}
                          {suggestion.tags.length > 0 && (
                            <div className='flex flex-wrap gap-1.5'>
                              {suggestion.tags.map((tag) => (
                                <span key={tag} className='tag text-xs'>{tag}</span>
                              ))}
                            </div>
                          )}
                          <button
                            onClick={() => handleSave(suggestion, i)}
                            disabled={savingIndex === i || savedIndices.has(i)}
                            className='btn-md btn-primary'
                          >
                            {savingIndex === i
                              ? 'Saving...'
                              : savedIndices.has(i)
                              ? 'Saved'
                              : 'Save This Drink'}
                          </button>
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        )}

        {/* Regenerate (AI only — recipe matches are deterministic) */}
        {suggestionSource !== 'recipes-only' && (
          <div className='card p-4 space-y-3'>
            <h4 className='text-sm font-semibold text-stone-700'>
              Want different suggestions?
            </h4>
            <textarea
              value={feedback}
              onChange={(e) => setFeedback(e.target.value)}
              placeholder='Optional: describe what you want differently (e.g., "more gin-based", "nothing too sweet")'
              className='input w-full'
              rows={2}
            />
            <div className='flex flex-wrap items-center gap-3'>
              <button
                onClick={handleRegenerate}
                disabled={aiMutation.isPending}
                className='btn-md btn-outline'
              >
                <IconRefresh className='w-4 h-4' />
                {aiMutation.isPending ? 'Generating...' : 'Regenerate'}
              </button>
              {modelsQuery.data && modelsQuery.data.length > 0 && (
                <select
                  value={modelQuery.data || 'claude-sonnet-4-20250514'}
                  onChange={(e) =>
                    setSetting.mutate({ key: 'claude_model', value: e.target.value })}
                  className='input-sm text-xs'
                >
                  {modelsQuery.data.map((model) => (
                    <option key={model.id} value={model.id}>{model.name}</option>
                  ))}
                </select>
              )}
            </div>
          </div>
        )}
      </div>
    )
  }

  // ─── Configure Phase ───────────────────────────────────────────

  const allBarSelected = (barItems?.length ?? 0) > 0
    && selectedBarIds.size === (barItems?.length ?? 0)
  const excludedCount = (barItems?.length ?? 0) - selectedBarIds.size

  return (
    <div className='p-6 space-y-4'>
      {/* API key check (hidden in recipes-only mode) */}
      {!hasApiKey && !apiKeyQuery.isLoading && suggestionSource !== 'recipes-only' && (
        <div className='panel-warning flex items-start gap-2'>
          <IconWarning className='w-5 h-5 text-amber-600 flex-shrink-0 mt-0.5' />
          <p className='text-sm text-amber-800'>
            No API key configured. Set your Anthropic API key in Settings to use AI suggestions.
          </p>
        </div>
      )}

      {/* Row 1: People + Style side by side (narrow | wide) */}
      <div className='grid grid-cols-1 md:grid-cols-[1fr_2fr] gap-4'>
        {/* Person selection */}
        {people && people.length > 0 && (
          <div className='card p-4'>
            <h3 className='font-semibold text-stone-900 mb-3'>Who&apos;s Drinking?</h3>
            <div className='space-y-2'>
              {[...people].sort((a, b) => computeAge(b.birthdate) - computeAge(a.birthdate)).map(
                (person) => {
                  const parsed = parsePerson(person)
                  const selected = selectedPersonIds.has(person.id)
                  const age = computeAge(person.birthdate)
                  const isMinor = age < 21
                  return (
                    <label
                      key={person.id}
                      className='flex items-start gap-2 cursor-pointer'
                    >
                      <input
                        type='checkbox'
                        checked={selected}
                        onChange={() => togglePerson(person.id)}
                        className='mt-1'
                      />
                      <div>
                        <span className='text-sm font-medium text-stone-800'>
                          {person.name}
                        </span>
                        {isMinor && (
                          <p className='text-xs text-amber-600'>
                            Under 21 — non-alcoholic only
                          </p>
                        )}
                        {parsed.drink_preferences.length > 0 && (
                          <p className='text-xs text-stone-400'>
                            Likes: {parsed.drink_preferences.join(', ')}
                          </p>
                        )}
                        {parsed.drink_dislikes.length > 0 && (
                          <p className='text-xs text-stone-400'>
                            Dislikes: {parsed.drink_dislikes.join(', ')}
                          </p>
                        )}
                      </div>
                    </label>
                  )
                },
              )}
            </div>
          </div>
        )}

        {/* Style selector (multi-select) */}
        <div className='card p-4'>
          <div className='flex items-center justify-between mb-3'>
            <h3 className='font-semibold text-stone-900'>
              Style {!isCustomMood && (
                <span className='text-sm font-normal text-stone-400'>
                  ({selectedStyles.size} / {STYLE_OPTIONS.length})
                </span>
              )}
            </h3>
            <button
              onClick={rollDice}
              className='text-xs text-stone-400 hover:text-stone-600 flex items-center gap-1'
              title='Randomly pick 2-3 styles'
            >
              🎲 Surprise me
            </button>
          </div>
          <div className='grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-2'>
            {STYLE_OPTIONS.map((opt) => {
              const active = !isCustomMood && selectedStyles.has(opt.label)
              return (
                <button
                  key={opt.label}
                  onClick={() => toggleStyle(opt.label)}
                  className={`flex flex-col items-center text-center px-2 py-2.5 rounded-lg border text-sm transition-colors ${
                    active
                      ? 'border-primary-300 bg-primary-50 text-primary-700'
                      : 'border-stone-200 bg-white text-stone-600 hover:border-stone-300 hover:text-stone-800'
                  }`}
                >
                  <span className='text-lg leading-none'>{opt.emoji}</span>
                  <span
                    className={`mt-1 text-xs leading-tight ${
                      active ? 'font-semibold' : 'font-medium'
                    }`}
                  >
                    {opt.label}
                  </span>
                  <span
                    className={`mt-0.5 text-[11px] leading-snug ${
                      active ? 'text-primary-500' : 'text-stone-400'
                    }`}
                  >
                    {opt.description}
                  </span>
                </button>
              )
            })}
            <button
              onClick={() => setIsCustomMood(true)}
              className={`flex flex-col items-center text-center px-2 py-2.5 rounded-lg border text-sm transition-colors ${
                isCustomMood
                  ? 'border-primary-300 bg-primary-50 text-primary-700'
                  : 'border-stone-200 bg-white text-stone-600 hover:border-stone-300 hover:text-stone-800'
              }`}
            >
              <span className='text-lg leading-none'>✨</span>
              <span
                className={`mt-1 text-xs leading-tight ${
                  isCustomMood ? 'font-semibold' : 'font-medium'
                }`}
              >
                Describe...
              </span>
            </button>
          </div>
          {isCustomMood && (
            <textarea
              value={customMoodText}
              onChange={(e) => setCustomMoodText(e.target.value)}
              placeholder='Describe the vibe (e.g., "something tart and citrusy for a warm evening")'
              className='input w-full mt-3'
              rows={3}
              autoFocus
            />
          )}
        </div>
      </div>

      {/* Row 2: Ingredients — collapsible */}
      <div className='card'>
        {(barItems?.length ?? 0) === 0
          ? (
            <div className='text-center p-4'>
              <p className='text-sm text-stone-500 mb-2'>
                No bar items yet. Add your ingredients first.
              </p>
              <Link to='/cocktails/bar' className='btn-sm btn-primary'>
                Set Up My Bar
              </Link>
            </div>
          )
          : (
            <>
              <button
                onClick={() => setShowIngredients(!showIngredients)}
                className='flex items-center justify-between w-full p-4 text-left'
              >
                <div className='flex items-center gap-2'>
                  <h3 className='font-semibold text-stone-900'>Ingredients</h3>
                  <span className='text-xs text-stone-400'>
                    {allBarSelected
                      ? `all ${barItems?.length} selected`
                      : `${selectedBarIds.size} of ${barItems?.length} — ${excludedCount} excluded`}
                  </span>
                </div>
                {showIngredients
                  ? <IconChevronDown className='w-4 h-4 text-stone-400' />
                  : <IconChevronRight className='w-4 h-4 text-stone-400' />}
              </button>
              {showIngredients && (
                <div className='px-4 pb-4'>
                  <div className='grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3'>
                    {BAR_ITEM_CATEGORIES.map((cat) => {
                      const items = groupedBar[cat.value]
                      if (!items || items.length === 0) return null
                      const allCatSelected = items.every((i) => selectedBarIds.has(i.id))
                      return (
                        <div
                          key={cat.value}
                          className='rounded-xl border border-stone-200 bg-white p-3 space-y-2'
                        >
                          <div className='flex items-center gap-1.5'>
                            <span className='text-base'>{cat.emoji}</span>
                            <h4 className='text-sm font-semibold text-stone-700'>
                              {cat.label}
                            </h4>
                            <span className='text-xs text-stone-400'>({items.length})</span>
                            <button
                              onClick={() => toggleAllBarCategory(items)}
                              className='text-xs text-stone-400 hover:text-stone-600 ml-auto'
                            >
                              {allCatSelected ? 'none' : 'all'}
                            </button>
                          </div>
                          <div className='flex flex-wrap gap-1'>
                            {items.map((item) => {
                              const selected = selectedBarIds.has(item.id)
                              return (
                                <button
                                  key={item.id}
                                  onClick={() => toggleBarItem(item.id)}
                                  className={`text-xs px-2 py-0.5 rounded-full cursor-pointer transition-colors ${
                                    selected
                                      ? cat.tagColor
                                      : 'bg-stone-100 text-stone-400 border border-stone-200 line-through'
                                  }`}
                                >
                                  {item.name}
                                </button>
                              )
                            })}
                          </div>
                        </div>
                      )
                    })}
                  </div>
                </div>
              )}
            </>
          )}
      </div>

      {/* Source toggle */}
      <div className='flex items-center gap-4'>
        <div className='flex gap-1 bg-stone-100 rounded-lg p-0.5'>
          {[
            { key: 'both' as const, label: 'Both' },
            { key: 'ai-only' as const, label: 'AI Only' },
            { key: 'recipes-only' as const, label: 'Recipes Only' },
          ].map(({ key, label }) => (
            <button
              key={key}
              onClick={() => setSuggestionSource(key)}
              className={`px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                suggestionSource === key
                  ? 'bg-white text-primary-700 shadow-sm'
                  : 'text-stone-500 hover:text-stone-700'
              }`}
            >
              {label}
            </button>
          ))}
        </div>
        <span className='text-xs text-stone-400'>
          {suggestionSource === 'both'
            ? 'Match saved recipes + generate new ideas'
            : suggestionSource === 'ai-only'
            ? 'AI-generated suggestions only'
            : 'Search your recipe book (no API key needed)'}
        </span>
      </div>

      {/* Generate row */}
      <div className='flex flex-wrap items-center gap-4'>
        <button
          onClick={() => handleGenerate()}
          disabled={(suggestionSource !== 'recipes-only' && !hasApiKey)
            || selectedBarIds.size === 0
            || selectedPersonIds.size === 0
            || aiMutation.isPending}
          className='btn-md btn-primary'
        >
          {aiMutation.isPending
            ? 'Generating...'
            : suggestionSource === 'recipes-only'
            ? 'Find Matching Recipes'
            : 'Suggest Drinks'}
        </button>
        {suggestionSource !== 'recipes-only' && modelsQuery.data && modelsQuery.data.length > 0 && (
          <select
            value={modelQuery.data || 'claude-sonnet-4-20250514'}
            onChange={(e) => setSetting.mutate({ key: 'claude_model', value: e.target.value })}
            className='input-sm text-xs'
          >
            {modelsQuery.data.map((model) => (
              <option key={model.id} value={model.id}>{model.name}</option>
            ))}
          </select>
        )}
      </div>

      {/* Error display */}
      {aiMutation.error && (
        <div className='panel-error text-red-700 text-sm'>
          {aiMutation.error instanceof Error ? aiMutation.error.message : String(aiMutation.error)}
        </div>
      )}
    </div>
  )
}

// ─── Recipe Match Card (local, not exported) ──────────────────

function RecipeMatchCard({
  recipe,
  isExpanded,
  onToggle,
}: {
  recipe: ParsedDrinkRecipe
  isExpanded: boolean
  onToggle: () => void
}) {
  return (
    <div className='card p-4 animate-slide-up'>
      <button
        onClick={onToggle}
        className='flex items-center gap-3 w-full text-left'
      >
        <span className='text-2xl flex-shrink-0'>{recipe.icon || '🍸'}</span>
        <div className='flex-1 min-w-0'>
          <h3 className='font-semibold text-stone-900'>{recipe.name}</h3>
          {recipe.description && (
            <p className='text-sm text-stone-500 line-clamp-2'>{recipe.description}</p>
          )}
          <div className='flex flex-wrap gap-1.5 mt-1'>
            {recipe.technique && <span className='tag text-xs'>{recipe.technique}</span>}
            {recipe.glassware && <span className='tag text-xs'>{recipe.glassware}</span>}
            {recipe.is_non_alcoholic && (
              <span className='tag text-xs bg-green-100 text-green-700'>non-alcoholic</span>
            )}
            {recipe.is_favorite && (
              <span className='tag text-xs bg-amber-100 text-amber-700'>favorite</span>
            )}
            {recipe.times_made > 0 && <span className='tag text-xs'>made {recipe.times_made}x
            </span>}
          </div>
        </div>
        {isExpanded
          ? <IconChevronDown className='w-4 h-4 text-stone-400' />
          : <IconChevronRight className='w-4 h-4 text-stone-400' />}
      </button>

      {isExpanded && (
        <div className='mt-4 pt-3 border-t border-stone-200 space-y-3 animate-fade-in'>
          <div>
            <h4 className='text-sm font-semibold text-stone-700 mb-1'>Ingredients</h4>
            <ul className='text-sm text-stone-600 space-y-0.5'>
              {recipe.ingredients.map((ing, j) => (
                <li key={j}>
                  {formatAmount(ing.amount)} {ing.unit} {ing.name}
                  {ing.notes && <span className='text-stone-400'>({ing.notes})</span>}
                </li>
              ))}
            </ul>
          </div>
          <div>
            <h4 className='text-sm font-semibold text-stone-700 mb-1'>Instructions</h4>
            <p className='text-sm text-stone-600 whitespace-pre-line'>{recipe.instructions}</p>
          </div>
          {recipe.garnish && (
            <p className='text-sm'>
              <span className='font-semibold text-stone-700'>Garnish:</span>{' '}
              <span className='text-stone-600'>{recipe.garnish}</span>
            </p>
          )}
          {recipe.tags.length > 0 && (
            <div className='flex flex-wrap gap-1.5'>
              {recipe.tags.map((tag) => <span key={tag} className='tag text-xs'>{tag}</span>)}
            </div>
          )}
          {recipe.notes && <p className='text-sm text-stone-500 italic'>{recipe.notes}</p>}
          {recipe.rating != null && recipe.rating > 0 && (
            <StarRating value={recipe.rating} size='sm' />
          )}
        </div>
      )}
    </div>
  )
}
