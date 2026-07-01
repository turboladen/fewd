# CLAUDE.md — Frontend (src/)

Frontend-specific guidance for the React 18 + TypeScript + Vite + TanStack Query + Tailwind app. The root `../CLAUDE.md` holds project-wide rules (cross-boundary conventions, CI, beads, session workflow) — read it too.

## Code Standards

### TypeScript/React

**Style:**

- Use dprint for formatting (enforced in CI)
- ESLint rules enforced in CI
- Prefer function components with hooks
- Use TypeScript strictly (no `any`)

**Component Structure:**

```typescript
// 1. Imports
import { useState } from 'react'
import { usePeople } from '../hooks/usePeople'

// 2. Types/Interfaces (if not in src/types/)
interface Props {
  onSave: () => void
}

// 3. Component
export function MyComponent({ onSave }: Props) {
  // 3a. Hooks
  const { data } = usePeople()
  const [isOpen, setIsOpen] = useState(false)
  
  // 3b. Event handlers
  const handleClick = () => {
    setIsOpen(true)
  }
  
  // 3c. Render helpers (if needed)
  const renderItem = (item: Item) => <div>{item.name}</div>
  
  // 3d. Early returns
  if (!data) return <div>Loading...</div>
  
  // 3e. Main render
  return <div onClick={handleClick}>...</div>
}
```

**State Management:**

- TanStack Query for server state (don’t duplicate in local state)
- `useState` for local UI state
- Avoid prop drilling (composition over props)
- No Redux/Zustand needed for this app

**Naming:**

- Components: PascalCase (e.g., `FamilyManager`)
- Hooks: camelCase with `use` prefix (e.g., `usePeople`)
- Event handlers: `handle` prefix (e.g., `handleClick`)
- Boolean props/state: `is/has/should` prefix (e.g., `isOpen`)

### Frontend Design System

**Tailwind v4 (CSS-first).** There is no `tailwind.config.js` — the theme lives in an
`@theme { … }` block in `src/index.css`, the build uses the `@tailwindcss/vite` plugin
(no PostCSS/autoprefixer), and `src/index.css` starts with `@import 'tailwindcss'`.
Design tokens are defined with the v4 `@utility` directive (not `@layer components`).
Every border in the app carries an explicit `border-*` color (tokens and component
classes alike), so v4's "bare `border` defaults to `currentColor`" change is a no-op
here — no border-color compatibility shim is needed. One Preflight shim remains in
`index.css`: a `button { cursor: pointer }` rule (v4 defaults buttons to
`cursor: default`). The browser baseline is Safari 16.4+ / Chrome 111+ / Firefox 128+
(set in `vite.config.ts` `build.target`).

**Typography:** Self-hosted variable fonts in `public/fonts/`:
- Headings: Playfair Display (serif) — `--font-heading` in the `@theme` block (`font-heading`)
- Body: DM Sans (sans-serif) — `--font-sans` override in the `@theme` block

**Design Tokens (`src/index.css`, defined via `@utility`):**

| Token | Usage |
|-------|-------|
| `.btn` + `.btn-xs`/`.btn-sm`/`.btn-md` | Button sizes (include focus ring + transition) |
| `.btn-primary`/`.btn-secondary`/`.btn-outline`/`.btn-ghost`/`.btn-danger` | Button variants |
| `.input`/`.input-sm` | Text inputs, selects, textareas |
| `.card`/`.card-hover` | Content containers with shadow + rounded-xl |
| `.tag` | Rounded-full pills for labels |
| `.panel-primary`/`.panel-secondary`/`.panel-warning`/`.panel-error` | Colored card variants |

**Animation Utilities (`src/index.css`, defined via `@utility`; keyframes in `@layer utilities`):**
- `animate-fade-in`, `animate-slide-up`, `animate-slide-down`, `animate-scale-in`, `animate-backdrop` — opacity + transform, GPU-composited
- `animate-expand` — height-based accordion reveal using `grid-template-rows: 0fr → 1fr`

**Shared UI Components:**

| Component | Purpose |
|-----------|---------|
| `Icon.tsx` | SVG icon components (Heroicons paths): `IconGear`, `IconClose`, `IconCheck`, `IconPlus`, `IconSearch`, `IconTrash`, `IconEdit`, `IconStar`/`IconStarFilled`, `IconArrowLeft`/`Right`, `IconChevronUp`/`Down`/`Left`/`Right`, `IconWarning`, `IconRefresh` |
| `Toast.tsx` | `ToastProvider` context + `useToast()` hook. Wrap app in provider, call `toast('message')` in mutation callbacks. |
| `EmptyState.tsx` | Centered empty-state display. Props: `emoji`, `title`, `description`, optional `action` |
| `TagInput.tsx` | Reusable tag editor. Props: `label`, `value`, `onChange`, optional `placeholder` |
| `StarRating.tsx` | Star rating display/input with SVG stars |
| `IngredientInput.tsx` | Reusable ingredient list editor (name, amount, unit, notes). Shared by food and drink recipe forms |
| `DrinkRecipeForm.tsx` | Drink recipe add/edit form. Reuses `IngredientInput` + `TagInput`. Types live in `src/types/drinkRecipe.ts` (not the component file) to satisfy `react-refresh/only-export-components` |

**Color Palette** (`@theme` CSS variables in `src/index.css`, e.g. `--color-primary-600`):
- `primary` — earthy greens (forest/sage tones)
- `secondary` — warm terracotta/copper
- `accent` — gold/amber highlights
- `surface` — warm off-white `#FDFAF6`

## Testing

### React Tests

**What to Test:**

- User interactions (clicks, form inputs)
- Data fetching states (loading, error, success)
- Conditional rendering

**Tools:**

- Vitest for test runner
- React Testing Library for component tests
- Mock API calls (fetch)

**Don’t Test:**

- Implementation details
- Third-party libraries
- Styling

**vitest doesn't run tsc.** Type errors in test code (e.g. factories missing newly-required fields, drifted prop types) pass silently in `bun run test`. Run `bunx tsc --noEmit` explicitly, or rely on the production build (`bun run build` chains tsc) to catch these. When expanding a shared type, sweep `src/test/factories.ts` for affected factories.

## Linting & Formatting

### TypeScript/React

```bash
# Format
dprint fmt

# Check formatting (CI)
dprint check

# Lint
bun run lint

# Lint fix
bun run lint:fix
```

## Key Patterns

### Frontend: Query + Mutation Pattern

```typescript
// Read data
const { data } = usePeople() // React Query

// Write data
const mutation = useCreatePerson()
mutation.mutate(newPerson)
```
