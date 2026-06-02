import Markdown, { type Components } from 'react-markdown'

/**
 * Renders recipe instruction markdown (headers, ordered/unordered lists, inline
 * **bold**) styled with the app's design tokens, so authored markdown displays
 * as formatted content instead of raw markers.
 *
 * Shared by the Recipe detail view (`detail`, small text), cook mode (`cook`,
 * large text), and the recipe/person Notes sections (`notes`, muted italic) so
 * they render consistently. Only inline + list/heading markdown is supported
 * (no raw HTML — react-markdown omits it by default).
 */
interface Props {
  markdown: string
  variant?: 'detail' | 'cook' | 'notes'
}

/**
 * Per-variant Tailwind classes for each element. `heading2`/`heading3` cover the
 * two heading scales (every `#`–`######` level maps onto one of them); `list` is
 * the shared `<ol>`/`<ul>` body — the factory prepends `list-decimal`/`list-disc`.
 * The bold-callout `<strong>` style is identical across variants, so it lives in
 * the factory rather than the table.
 */
interface VariantClasses {
  heading2: string
  heading3: string
  body: string
  list: string
}

function makeComponents({ heading2, heading3, body, list }: VariantClasses): Components {
  return {
    h1: ({ children }) => <h1 className={heading2}>{children}</h1>,
    h2: ({ children }) => <h2 className={heading2}>{children}</h2>,
    h3: ({ children }) => <h3 className={heading3}>{children}</h3>,
    h4: ({ children }) => <h4 className={heading3}>{children}</h4>,
    h5: ({ children }) => <h5 className={heading3}>{children}</h5>,
    h6: ({ children }) => <h6 className={heading3}>{children}</h6>,
    p: ({ children }) => <p className={body}>{children}</p>,
    ol: ({ children }) => <ol className={`list-decimal ${list}`}>{children}</ol>,
    ul: ({ children }) => <ul className={`list-disc ${list}`}>{children}</ul>,
    strong: ({ children }) => <strong className='text-primary-700 font-semibold'>{children}
    </strong>,
  }
}

const VARIANTS: Record<NonNullable<Props['variant']>, Components> = {
  detail: makeComponents({
    heading2: 'font-heading text-lg font-semibold text-stone-900 mt-4 mb-1 first:mt-0',
    heading3: 'font-heading text-base font-semibold text-stone-800 mt-3 mb-1',
    body: 'text-sm text-stone-700 leading-relaxed mb-2 last:mb-0',
    list: 'pl-5 space-y-1 text-sm text-stone-700 leading-relaxed mb-2 last:mb-0',
  }),
  cook: makeComponents({
    heading2: 'font-heading text-2xl md:text-3xl font-semibold text-stone-900 mt-2 mb-3',
    heading3: 'font-heading text-xl md:text-2xl font-semibold text-stone-800 mt-2 mb-2',
    body: 'text-lg md:text-xl leading-relaxed text-stone-800',
    list: 'pl-6 space-y-2 text-lg md:text-xl leading-relaxed text-stone-800',
  }),
  // Muted + italic to match the Notes section's existing voice; headings stay
  // upright so a noted section label still reads as a label.
  notes: makeComponents({
    heading2: 'font-heading text-base font-semibold text-stone-700 mt-3 mb-1 first:mt-0',
    heading3: 'font-heading text-sm font-semibold text-stone-700 mt-2 mb-1',
    body: 'text-sm text-stone-600 italic leading-relaxed mb-2 last:mb-0',
    list: 'pl-5 space-y-1 text-sm text-stone-600 italic leading-relaxed mb-2 last:mb-0',
  }),
}

export function RecipeMarkdown({ markdown, variant = 'detail' }: Props) {
  return <Markdown components={VARIANTS[variant]}>{markdown}</Markdown>
}
