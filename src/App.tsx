import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { createBrowserRouter, Navigate, RouterProvider } from 'react-router-dom'
import { BarInventory } from './components/BarInventory'
import { CocktailSuggester } from './components/CocktailSuggester'
import { DrinkRecipeManager } from './components/DrinkRecipeManager'
import { FamilyManager } from './components/FamilyManager'
import { MealPlanner } from './components/MealPlanner'
import { RecipeManager } from './components/RecipeManager'
import { ShoppingList } from './components/ShoppingList'
import { TemplateManager } from './components/TemplateManager'
import { ToastProvider } from './components/Toast'
import { ChromeProvider } from './contexts/ChromeContext'
import { DrinkRecipeDetailPage } from './routes/DrinkRecipeDetailPage'
import { RecipeDetailPage } from './routes/RecipeDetailPage'
import { RootLayout } from './routes/RootLayout'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
    },
  },
})

const router = createBrowserRouter([
  {
    path: '/',
    element: <RootLayout />,
    children: [
      { index: true, element: <Navigate to='/family' replace /> },
      { path: 'family', element: <FamilyManager /> },
      { path: 'meals', element: <Navigate to='/meals/planner' replace /> },
      { path: 'meals/planner', element: <MealPlanner /> },
      { path: 'meals/templates', element: <TemplateManager /> },
      { path: 'meals/shopping', element: <ShoppingList /> },
      { path: 'recipes', element: <RecipeManager /> },
      { path: 'recipes/:id', element: <RecipeDetailPage /> },
      { path: 'cocktails', element: <Navigate to='/cocktails/suggest' replace /> },
      { path: 'cocktails/suggest', element: <CocktailSuggester /> },
      { path: 'cocktails/recipes', element: <DrinkRecipeManager /> },
      { path: 'cocktails/recipes/:id', element: <DrinkRecipeDetailPage /> },
      { path: 'cocktails/bar', element: <BarInventory /> },
    ],
  },
])

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <ChromeProvider>
          <RouterProvider router={router} />
        </ChromeProvider>
      </ToastProvider>
    </QueryClientProvider>
  )
}

export default App
