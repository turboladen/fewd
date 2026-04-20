import { screen } from '@testing-library/react'
import { useEffect } from 'react'
import { Route, Routes } from 'react-router-dom'
import { describe, expect, it } from 'vitest'
import { useChrome } from '../contexts/ChromeContext'
import { renderWithProviders } from '../test/renderWithProviders'
import { RootLayout } from './RootLayout'

function ChromeHider() {
  const { setHidden } = useChrome()
  useEffect(() => {
    setHidden(true)
    return () => setHidden(false)
  }, [setHidden])
  return <div>focused-content</div>
}

describe('RootLayout', () => {
  it('renders the top nav by default', () => {
    renderWithProviders(
      <Routes>
        <Route path='/' element={<RootLayout />}>
          <Route index element={<div>home</div>} />
        </Route>
      </Routes>,
    )
    expect(screen.getByRole('link', { name: 'Recipes' })).toBeInTheDocument()
  })

  it('renders the meals sub-nav when on a /meals route', () => {
    renderWithProviders(
      <Routes>
        <Route path='/' element={<RootLayout />}>
          <Route path='meals/planner' element={<div>planner</div>} />
        </Route>
      </Routes>,
      { initialPath: '/meals/planner' },
    )
    expect(screen.getByRole('link', { name: 'Planner' })).toBeInTheDocument()
  })

  it('hides both top nav and sub-nav when a child sets the chrome flag', () => {
    renderWithProviders(
      <Routes>
        <Route path='/' element={<RootLayout />}>
          <Route path='meals/planner' element={<ChromeHider />} />
        </Route>
      </Routes>,
      { initialPath: '/meals/planner' },
    )
    expect(screen.queryByRole('link', { name: 'Recipes' })).not.toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Planner' })).not.toBeInTheDocument()
    expect(screen.getByText('focused-content')).toBeInTheDocument()
  })
})
