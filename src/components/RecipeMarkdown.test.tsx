import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { RecipeMarkdown } from './RecipeMarkdown'

describe('RecipeMarkdown', () => {
  it('renders ## and ### as heading elements, not literal markers', () => {
    render(<RecipeMarkdown markdown={'## Custard Base\n\n### Sub note'} />)
    expect(screen.getByRole('heading', { level: 2, name: 'Custard Base' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 3, name: 'Sub note' })).toBeInTheDocument()
    expect(screen.queryByText(/#/)).not.toBeInTheDocument()
  })

  it('styles every heading level so none falls back to an unstyled browser default', () => {
    render(<RecipeMarkdown markdown={'# Top\n\n#### Deep'} />)
    // h1 and h4 are covered by the variant maps (mapped onto the two scales),
    // so they carry design-token classes rather than rendering bare.
    expect(screen.getByRole('heading', { level: 1, name: 'Top' }).className).toContain(
      'font-heading',
    )
    expect(screen.getByRole('heading', { level: 4, name: 'Deep' }).className).toContain(
      'font-heading',
    )
  })

  it('renders numbered and bulleted lists as list items, not literal markers', () => {
    render(<RecipeMarkdown markdown={'1. First\n2. Second'} />)
    const items = screen.getAllByRole('listitem')
    expect(items).toHaveLength(2)
    expect(items[0]).toHaveTextContent('First')
    expect(screen.queryByText(/^1\./)).not.toBeInTheDocument()
  })

  it('renders inline **bold** (enhanced ingredient amounts) as <strong>', () => {
    render(<RecipeMarkdown markdown={'Melt **2 tbsp butter** in a pan.'} />)
    const bold = screen.getByText('2 tbsp butter')
    expect(bold.tagName).toBe('STRONG')
  })

  it('renders the notes variant as markdown with muted italic body text', () => {
    render(<RecipeMarkdown markdown={'Best **chilled**.\n\n- make ahead'} variant='notes' />)
    expect(screen.getByText('chilled').tagName).toBe('STRONG')
    expect(screen.getAllByRole('listitem')[0]).toHaveTextContent('make ahead')
    const paragraph = screen.getByText('chilled').closest('p')
    expect(paragraph?.className).toContain('italic')
  })
})
