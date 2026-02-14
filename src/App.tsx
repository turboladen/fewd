import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useState } from 'react'
import { BarInventory } from './components/BarInventory'
import { CocktailSuggester } from './components/CocktailSuggester'
import { DrinkRecipeManager } from './components/DrinkRecipeManager'
import { FamilyManager } from './components/FamilyManager'
import { IconGear } from './components/Icon'
import { MealPlanner } from './components/MealPlanner'
import { RecipeManager } from './components/RecipeManager'
import { SettingsPanel } from './components/SettingsPanel'
import { ShoppingList } from './components/ShoppingList'
import { TemplateManager } from './components/TemplateManager'
import { ToastProvider } from './components/Toast'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
    },
  },
})

type TopTab = 'family' | 'meals' | 'cocktails'
type MealsSubTab = 'recipes' | 'planner' | 'templates' | 'shopping'
type CocktailsSubTab = 'suggest' | 'drink-recipes' | 'bar'

const topTabs: { key: TopTab; label: string }[] = [
  { key: 'family', label: 'Family' },
  { key: 'meals', label: 'Meals' },
  { key: 'cocktails', label: 'Cocktails' },
]

const mealsSubTabs: { key: MealsSubTab; label: string }[] = [
  { key: 'recipes', label: 'Recipes' },
  { key: 'planner', label: 'Planner' },
  { key: 'templates', label: 'Templates' },
  { key: 'shopping', label: 'Shopping' },
]

const cocktailsSubTabs: { key: CocktailsSubTab; label: string }[] = [
  { key: 'suggest', label: 'Suggest' },
  { key: 'drink-recipes', label: 'Recipes' },
  { key: 'bar', label: 'My Bar' },
]

function SubNav<T extends string>({
  tabs,
  activeTab,
  onTabChange,
}: {
  tabs: { key: T; label: string }[]
  activeTab: T
  onTabChange: (tab: T) => void
}) {
  return (
    <div className='flex-none bg-white border-b border-stone-200/60 px-3 sm:px-6 py-2'>
      <div className='flex gap-1 bg-stone-100 rounded-lg p-0.5 w-fit max-w-full overflow-x-auto'>
        {tabs.map(({ key, label }) => (
          <button
            key={key}
            onClick={() => onTabChange(key)}
            className={`px-2 sm:px-3 py-1.5 rounded-md text-xs sm:text-sm font-medium transition-colors ${
              activeTab === key
                ? 'bg-white text-primary-700 shadow-sm'
                : 'text-stone-500 hover:text-stone-700'
            }`}
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  )
}

function App() {
  const [activeTopTab, setActiveTopTab] = useState<TopTab>('family')
  const [activeMealsSubTab, setActiveMealsSubTab] = useState<MealsSubTab>('recipes')
  const [activeCocktailsSubTab, setActiveCocktailsSubTab] = useState<CocktailsSubTab>('suggest')
  const [isSettingsOpen, setIsSettingsOpen] = useState(false)

  const contentKey = activeTopTab === 'meals'
    ? `meals-${activeMealsSubTab}`
    : activeTopTab === 'cocktails'
    ? `cocktails-${activeCocktailsSubTab}`
    : activeTopTab

  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <div className='h-screen flex flex-col bg-surface'>
          {/* Top nav */}
          <nav className='flex-none bg-white/95 backdrop-blur-sm border-b border-stone-200/80 shadow-soft'>
            <div className='flex pl-2'>
              {topTabs.map(({ key, label }) => (
                <button
                  key={key}
                  onClick={() => setActiveTopTab(key)}
                  className={`relative px-3 sm:px-6 py-3 font-medium text-sm transition-colors duration-150 ${
                    activeTopTab === key
                      ? 'text-primary-600'
                      : 'text-stone-500 hover:text-stone-800'
                  }`}
                >
                  {label}
                  {activeTopTab === key && (
                    <span className='absolute bottom-0 left-2 right-2 h-0.5 bg-primary-500 rounded-full' />
                  )}
                </button>
              ))}
              <div className='ml-auto pr-2 sm:pr-4 flex items-center'>
                <button
                  onClick={() => setIsSettingsOpen(true)}
                  className='btn-ghost text-stone-400 hover:text-stone-600'
                  title='Settings'
                  aria-label='Settings'
                >
                  <IconGear className='w-5 h-5' />
                </button>
              </div>
            </div>
          </nav>

          {/* Sub-nav */}
          {activeTopTab === 'meals' && (
            <SubNav
              tabs={mealsSubTabs}
              activeTab={activeMealsSubTab}
              onTabChange={setActiveMealsSubTab}
            />
          )}
          {activeTopTab === 'cocktails' && (
            <SubNav
              tabs={cocktailsSubTabs}
              activeTab={activeCocktailsSubTab}
              onTabChange={setActiveCocktailsSubTab}
            />
          )}

          {/* Content */}
          <main className='flex-1 overflow-y-auto'>
            <div key={contentKey} className='animate-fade-in'>
              {activeTopTab === 'family' && <FamilyManager />}
              {activeTopTab === 'meals' && activeMealsSubTab === 'recipes' && <RecipeManager />}
              {activeTopTab === 'meals' && activeMealsSubTab === 'planner' && <MealPlanner />}
              {activeTopTab === 'meals' && activeMealsSubTab === 'templates' && <TemplateManager />}
              {activeTopTab === 'meals' && activeMealsSubTab === 'shopping' && <ShoppingList />}
              {activeTopTab === 'cocktails' && activeCocktailsSubTab === 'suggest' && (
                <CocktailSuggester onSwitchToBar={() => setActiveCocktailsSubTab('bar')} />
              )}
              {activeTopTab === 'cocktails' && activeCocktailsSubTab === 'drink-recipes' && (
                <DrinkRecipeManager onSwitchToSuggest={() => setActiveCocktailsSubTab('suggest')} />
              )}
              {activeTopTab === 'cocktails' && activeCocktailsSubTab === 'bar' && <BarInventory />}
            </div>
          </main>
          {isSettingsOpen && <SettingsPanel onClose={() => setIsSettingsOpen(false)} />}
        </div>
      </ToastProvider>
    </QueryClientProvider>
  )
}

export default App
