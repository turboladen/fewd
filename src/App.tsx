import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useState } from 'react'
import { FamilyManager } from './components/FamilyManager'
import { MealPlanner } from './components/MealPlanner'
import { RecipeManager } from './components/RecipeManager'
import { ShoppingList } from './components/ShoppingList'

const queryClient = new QueryClient()

type Tab = 'family' | 'recipes' | 'planner' | 'shopping'

function App() {
  const [activeTab, setActiveTab] = useState<Tab>('family')

  return (
    <QueryClientProvider client={queryClient}>
      <div className='h-screen flex flex-col bg-gray-50'>
        <nav
          className='flex-none bg-white border-b border-gray-200 pt-7'
          data-tauri-drag-region
        >
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
              onClick={() => setActiveTab('shopping')}
              className={`px-6 py-3 font-medium text-sm ${
                activeTab === 'shopping'
                  ? 'text-blue-600 border-b-2 border-blue-600'
                  : 'text-gray-600 hover:text-gray-900'
              }`}
            >
              Shopping
            </button>
          </div>
        </nav>
        <main className='flex-1 overflow-y-auto'>
          {activeTab === 'family' && <FamilyManager />}
          {activeTab === 'recipes' && <RecipeManager />}
          {activeTab === 'planner' && <MealPlanner />}
          {activeTab === 'shopping' && <ShoppingList />}
        </main>
      </div>
    </QueryClientProvider>
  )
}

export default App
