import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useState } from 'react'
import { FamilyManager } from './components/FamilyManager'
import { RecipeManager } from './components/RecipeManager'

const queryClient = new QueryClient()

type Tab = 'family' | 'recipes'

function App() {
  const [activeTab, setActiveTab] = useState<Tab>('family')

  return (
    <QueryClientProvider client={queryClient}>
      <div className='min-h-screen bg-gray-50'>
        <nav className='bg-white border-b border-gray-200'>
          <div className='flex'>
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
          </div>
        </nav>
        {activeTab === 'family' && <FamilyManager />}
        {activeTab === 'recipes' && <RecipeManager />}
      </div>
    </QueryClientProvider>
  )
}

export default App
