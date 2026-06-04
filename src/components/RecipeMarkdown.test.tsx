import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { RecipeMarkdown } from './RecipeMarkdown'

// RecipeMarkdown lazy-loads its impl (react-markdown) behind a Suspense
// boundary, so the first query in each test must be async (`findBy*`) to let the
// dynamic import resolve; later queries are synchronous once it has rendered.
describe('RecipeMarkdown', () => {
  it('renders ## and ### as heading elements, not literal markers', async () => {
    render(<RecipeMarkdown markdown={'## Custard Base\n\n### Sub note'} />)
    expect(await screen.findByRole('heading', { level: 2, name: 'Custard Base' }))
      .toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 3, name: 'Sub note' })).toBeInTheDocument()
    expect(screen.queryByText(/#/)).not.toBeInTheDocument()
  })

  it('styles every heading level so none falls back to an unstyled browser default', async () => {
    render(<RecipeMarkdown markdown={'# Top\n\n#### Deep'} />)
    // h1 and h4 are covered by the variant maps (mapped onto the two scales),
    // so they carry design-token classes rather than rendering bare.
    const h1 = await screen.findByRole('heading', { level: 1, name: 'Top' })
    expect(h1.className).toContain('font-heading')
    expect(screen.getByRole('heading', { level: 4, name: 'Deep' }).className).toContain(
      'font-heading',
    )
  })

  it('renders numbered and bulleted lists as list items, not literal markers', async () => {
    render(<RecipeMarkdown markdown={'1. First\n2. Second'} />)
    const items = await screen.findAllByRole('listitem')
    expect(items).toHaveLength(2)
    expect(items[0]).toHaveTextContent('First')
    expect(screen.queryByText(/^1\./)).not.toBeInTheDocument()
  })

  it('renders inline **bold** (enhanced ingredient amounts) as <strong>', async () => {
    render(<RecipeMarkdown markdown={'Melt **2 tbsp butter** in a pan.'} />)
    const bold = await screen.findByText('2 tbsp butter')
    expect(bold.tagName).toBe('STRONG')
  })

  it('renders the notes variant as markdown with muted italic body text', async () => {
    render(<RecipeMarkdown markdown={'Best **chilled**.\n\n- make ahead'} variant='notes' />)
    const bold = await screen.findByText('chilled')
    expect(bold.tagName).toBe('STRONG')
    expect(screen.getAllByRole('listitem')[0]).toHaveTextContent('make ahead')
    expect(bold.closest('p')?.className).toContain('italic')
  })
})
