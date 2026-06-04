import { lazy, Suspense } from 'react'
import { ErrorBoundary } from './ErrorBoundary'
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

/**
 * `Suspense` covers the chunk's *loading* state, but not a *failed* load — a
 * rejected dynamic import (e.g. a stale chunk hash after a redeploy with a tab
 * open) throws past Suspense to the nearest error boundary. Without a local one
 * that would be the app-root `ErrorBoundary`, white-screening the whole app just
 * because one recipe's markdown chunk 404'd. The inline `ErrorBoundary` here
 * degrades to the raw markdown text instead, so the recipe stays readable.
 *
 * `Suspense fallback={null}`: content is already in memory and the chunk loads
 * fast, so a momentary blank beats flashing raw markers. Once the chunk resolves
 * it is cached, so later instances (e.g. cook mode's per-step renders) are sync.
 */
export function RecipeMarkdown(props: Props) {
  return (
    <ErrorBoundary fallback={<p className='whitespace-pre-wrap'>{props.markdown}</p>}>
      <Suspense fallback={null}>
        <RecipeMarkdownImpl {...props} />
      </Suspense>
    </ErrorBoundary>
  )
}
