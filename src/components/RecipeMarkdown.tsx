import { lazy, Suspense } from 'react'
import type { Props } from './RecipeMarkdownImpl'

/**
 * Lazy boundary for the markdown renderer. `react-markdown` and its parser tree
 * (micromark/mdast/unified — the bulk of the JS bundle) are deferred behind a
 * dynamic import so they split into a separate chunk that only loads when a
 * recipe/notes/cook view first renders, not on initial page load (fewd-0fq).
 *
 * The public name + import path are unchanged, so every callsite stays the same.
 */
const RecipeMarkdownImpl = lazy(() => import('./RecipeMarkdownImpl'))

// `fallback={null}`: content is already in memory and the chunk loads fast, so a
// momentary blank beats flashing raw markdown markers. Once the chunk resolves
// it is cached, so later instances (e.g. cook mode's per-step renders) are sync.
export function RecipeMarkdown(props: Props) {
  return (
    <Suspense fallback={null}>
      <RecipeMarkdownImpl {...props} />
    </Suspense>
  )
}
