import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useState } from 'react'
import { FamilyManager } from './components/FamilyManager'
import { IconGear } from './components/Icon'
import { MealPlanner } from './components/MealPlanner'
import { RecipeManager } from './components/RecipeManager'
import { SettingsPanel } from './components/SettingsPanel'
import { ShoppingList } from './components/ShoppingList'
import { TemplateManager } from './components/TemplateManager'
import { ToastProvider } from './components/Toast'

const queryClient = new QueryClient()

type Tab = 'family' | 'recipes' | 'planner' | 'templates' | 'shopping'

const tabs: { key: Tab; label: string }[] = [
  { key: 'family', label: 'Family' },
  { key: 'recipes', label: 'Recipes' },
  { key: 'planner', label: 'Planner' },
  { key: 'templates', label: 'Templates' },
  { key: 'shopping', label: 'Shopping' },
]

function App() {
  const [activeTab, setActiveTab] = useState<Tab>('family')
  const [isSettingsOpen, setIsSettingsOpen] = useState(false)

  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <div className='h-screen flex flex-col bg-surface'>
          <nav className='flex-none bg-white/95 backdrop-blur-sm border-b border-stone-200/80 shadow-soft'>
            <div className='flex pl-2'>
              {tabs.map(({ key, label }) => (
                <button
                  key={key}
                  onClick={() => setActiveTab(key)}
                  className={`relative px-6 py-3 font-medium text-sm transition-colors duration-150 ${
                    activeTab === key
                      ? 'text-primary-600'
                      : 'text-stone-500 hover:text-stone-800'
                  }`}
                >
                  {label}
                  {activeTab === key && (
                    <span className='absolute bottom-0 left-2 right-2 h-0.5 bg-primary-500 rounded-full' />
                  )}
                </button>
              ))}
              <div className='ml-auto pr-4 flex items-center'>
                <button
                  onClick={() => setIsSettingsOpen(true)}
                  className='btn-ghost text-stone-400 hover:text-stone-600'
                  title='Settings'
                >
                  <IconGear className='w-5 h-5' />
                </button>
              </div>
            </div>
          </nav>
          <main className='flex-1 overflow-y-auto'>
            <div key={activeTab} className='animate-fade-in'>
              {activeTab === 'family' && <FamilyManager />}
              {activeTab === 'recipes' && <RecipeManager />}
              {activeTab === 'planner' && <MealPlanner />}
              {activeTab === 'templates' && <TemplateManager />}
              {activeTab === 'shopping' && <ShoppingList />}
            </div>
          </main>
          {isSettingsOpen && <SettingsPanel onClose={() => setIsSettingsOpen(false)} />}
        </div>
      </ToastProvider>
    </QueryClientProvider>
  )
}

export default App
