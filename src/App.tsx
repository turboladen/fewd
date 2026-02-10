import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useState } from 'react'
import { FamilyManager } from './components/FamilyManager'
import { LockWarningBanner } from './components/LockWarningBanner'
import { MealPlanner } from './components/MealPlanner'
import { RecipeManager } from './components/RecipeManager'
import { SettingsPanel } from './components/SettingsPanel'
import { ShoppingList } from './components/ShoppingList'
import { TemplateManager } from './components/TemplateManager'

const queryClient = new QueryClient()

type Tab = 'family' | 'recipes' | 'planner' | 'templates' | 'shopping'

function App() {
  const [activeTab, setActiveTab] = useState<Tab>('family')
  const [isSettingsOpen, setIsSettingsOpen] = useState(false)

  return (
    <QueryClientProvider client={queryClient}>
      <div className='h-screen flex flex-col bg-gray-50'>
        <nav className='flex-none bg-white border-b border-gray-200'>
          <div
            className='h-7 w-full'
            data-tauri-drag-region
          />
          <div className='flex pl-2'>
            <button
              onClick={() => setActiveTab('family')}
              className={`px-6 py-3 font-medium text-sm ${
                activeTab === 'family'
                  ? 'text-blue-600 border-b-2 border-blue-600'
                  : 'text-gray-600 hover:text-gray-900'
              }`}
            >
              Family
            </button>
            <button
              onClick={() => setActiveTab('recipes')}
              className={`px-6 py-3 font-medium text-sm ${
                activeTab === 'recipes'
                  ? 'text-blue-600 border-b-2 border-blue-600'
                  : 'text-gray-600 hover:text-gray-900'
              }`}
            >
              Recipes
            </button>
            <button
              onClick={() => setActiveTab('planner')}
              className={`px-6 py-3 font-medium text-sm ${
                activeTab === 'planner'
                  ? 'text-blue-600 border-b-2 border-blue-600'
                  : 'text-gray-600 hover:text-gray-900'
              }`}
            >
              Planner
            </button>
            <button
              onClick={() => setActiveTab('templates')}
              className={`px-6 py-3 font-medium text-sm ${
                activeTab === 'templates'
                  ? 'text-blue-600 border-b-2 border-blue-600'
                  : 'text-gray-600 hover:text-gray-900'
              }`}
            >
              Templates
            </button>
            <button
              onClick={() => setActiveTab('shopping')}
              className={`px-6 py-3 font-medium text-sm ${
                activeTab === 'shopping'
                  ? 'text-blue-600 border-b-2 border-blue-600'
                  : 'text-gray-600 hover:text-gray-900'
              }`}
            >
              Shopping
            </button>
            <div className='ml-auto pr-4 flex items-center'>
              <button
                onClick={() => setIsSettingsOpen(true)}
                className='text-gray-400 hover:text-gray-600 p-2 text-lg'
                title='Settings'
              >
                {'\u2699'}
              </button>
            </div>
          </div>
        </nav>
        <LockWarningBanner />
        <main className='flex-1 overflow-y-auto'>
          {activeTab === 'family' && <FamilyManager />}
          {activeTab === 'recipes' && <RecipeManager />}
          {activeTab === 'planner' && <MealPlanner />}
          {activeTab === 'templates' && <TemplateManager />}
          {activeTab === 'shopping' && <ShoppingList />}
        </main>
        {isSettingsOpen && <SettingsPanel onClose={() => setIsSettingsOpen(false)} />}
      </div>
    </QueryClientProvider>
  )
}

export default App
