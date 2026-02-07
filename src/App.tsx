import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { FamilyManager } from './components/FamilyManager'

const queryClient = new QueryClient()

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <div className='min-h-screen bg-gray-50'>
        <FamilyManager />
      </div>
    </QueryClientProvider>
  )
}

export default App
