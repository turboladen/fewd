import { useState } from 'react'
import { NavLink, Outlet, useLocation } from 'react-router-dom'
import { IconGear } from '../components/Icon'
import { SettingsPanel } from '../components/SettingsPanel'
import { useChrome } from '../contexts/ChromeContext'

const topTabs = [
  { to: '/family', label: 'Family' },
  { to: '/meals', label: 'Meals' },
  { to: '/recipes', label: 'Recipes' },
  { to: '/cocktails', label: 'Cocktails' },
] as const

const mealsSubTabs = [
  { to: '/meals/planner', label: 'Planner' },
  { to: '/meals/templates', label: 'Templates' },
  { to: '/meals/shopping', label: 'Shopping' },
] as const

const cocktailsSubTabs = [
  { to: '/cocktails/suggest', label: 'Suggest' },
  { to: '/cocktails/recipes', label: 'Recipes' },
  { to: '/cocktails/bar', label: 'My Bar' },
] as const

function SubNav({ tabs }: { tabs: readonly { to: string; label: string }[] }) {
  return (
    <div className='flex-none bg-white border-b border-stone-200/60 px-3 sm:px-6 py-2'>
      <div className='flex gap-1 bg-stone-100 rounded-lg p-0.5 w-fit max-w-full overflow-x-auto'>
        {tabs.map(({ to, label }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `px-2 sm:px-3 py-1.5 rounded-md text-xs sm:text-sm font-medium transition-colors ${
                isActive
                  ? 'bg-white text-primary-700 shadow-sm'
                  : 'text-stone-500 hover:text-stone-700'
              }`}
          >
            {label}
          </NavLink>
        ))}
      </div>
    </div>
  )
}

export function RootLayout() {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false)
  const location = useLocation()
  const { isHidden: isChromeHidden } = useChrome()
  const inMeals = location.pathname === '/meals' || location.pathname.startsWith('/meals/')
  const inCocktails = location.pathname === '/cocktails'
    || location.pathname.startsWith('/cocktails/')

  return (
    <div className='h-screen flex flex-col bg-surface'>
      {!isChromeHidden && (
        <nav className='flex-none bg-white/95 backdrop-blur-sm border-b border-stone-200/80 shadow-soft'>
          <div className='flex pl-2'>
            {topTabs.map(({ to, label }) => (
              <NavLink
                key={to}
                to={to}
                className={({ isActive }) =>
                  `relative px-3 sm:px-6 py-3 font-medium text-sm transition-colors duration-150 ${
                    isActive
                      ? 'text-primary-600'
                      : 'text-stone-500 hover:text-stone-800'
                  }`}
              >
                {({ isActive }) => (
                  <>
                    {label}
                    {isActive && (
                      <span className='absolute bottom-0 left-2 right-2 h-0.5 bg-primary-500 rounded-full' />
                    )}
                  </>
                )}
              </NavLink>
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
      )}

      {!isChromeHidden && inMeals && <SubNav tabs={mealsSubTabs} />}
      {!isChromeHidden && inCocktails && <SubNav tabs={cocktailsSubTabs} />}

      <main className='flex-1 overflow-y-auto'>
        <div key={location.pathname} className='animate-fade-in'>
          <Outlet />
        </div>
      </main>
      {isSettingsOpen && <SettingsPanel onClose={() => setIsSettingsOpen(false)} />}
    </div>
  )
}
