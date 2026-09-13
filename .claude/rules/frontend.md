---
paths:
  - "src/**"
  - "public/**"
  - "index.html"
  - "vite.config.ts"
  - "package.json"
  - "bun.lock"
---

# Frontend

React 19 + TypeScript + Vite + TanStack Query + React Router 7 + Tailwind v4.

- Routes are defined with `createBrowserRouter` in `src/App.tsx`. Top-level tabs are Family | Meals | Recipes | Cocktails; Meals has Planner | Templates | Shopping and Cocktails has Suggest | Recipes | My Bar, rendered by `SubNav` in `src/routes/RootLayout.tsx`.
- TanStack Query owns server state (don't copy it into local state); `useState` holds local UI state. Hooks live in `src/hooks/`, shared types in `src/types/`.
- TypeScript is strict: no `any`.

## Design system

**Tailwind v4, CSS-first.** There is no `tailwind.config.js`: the theme lives in an `@theme { … }` block in `src/index.css`, the build uses the `@tailwindcss/vite` plugin (no PostCSS/autoprefixer), and `src/index.css` starts with `@import 'tailwindcss'`. Design tokens are defined with the `@utility` directive, not `@layer components`. One Preflight shim remains in `index.css`: `button { cursor: pointer }`, because v4 defaults buttons to `cursor: default`. The browser baseline is Safari 16.4+ / Chrome 111+ / Firefox 128+ (`vite.config.ts` `build.target`).

**Typography:** self-hosted variable fonts in `public/fonts/`. Headings use Playfair Display (`--font-heading`, `font-heading`); body text uses DM Sans (`--font-sans`).

**Tokens** (`src/index.css`, via `@utility`):

| Token                                                                                         | Usage                                            |
| --------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| `.btn` + `.btn-xs`/`.btn-sm`/`.btn-md`                                                        | Button sizes (focus ring and transition)         |
| `.btn-primary`/`.btn-secondary`/`.btn-outline`/`.btn-ghost`/`.btn-danger`/`.btn-danger-solid` | Button variants                                  |
| `.input`/`.input-sm`                                                                          | Text inputs, selects, textareas                  |
| `.card`/`.card-hover`                                                                         | Content containers with shadow and `rounded-xl`  |
| `.tag`/`.tag-remove`                                                                          | Rounded-full label pills and their remove button |
| `.panel-primary`/`.panel-secondary`/`.panel-warning`/`.panel-error`                           | Colored card variants                            |

**Animations** (`@utility`, keyframes in `@layer utilities`): `animate-fade-in`, `animate-slide-up`, `animate-slide-down`, `animate-scale-in`, and `animate-backdrop` are opacity and transform, GPU-composited; `animate-expand` is a height accordion using `grid-template-rows: 0fr → 1fr`.

**Colors** (`@theme` variables such as `--color-primary-600`): `primary` earthy greens, `secondary` warm terracotta, `accent` gold/amber, `surface` warm off-white `#FDFAF6`.

**Shared components** (`src/components/`):

| Component             | Purpose                                                                                                                |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `Icon.tsx`            | SVG icons (Heroicons paths), e.g. `IconGear`, `IconClose`, `IconX`, `IconPlus`, `IconTrash`, `IconStar`, `IconPrinter` |
| `Toast.tsx`           | `ToastProvider` and `useToast()`; call `toast('message')` in mutation callbacks                                        |
| `EmptyState.tsx`      | Centered empty state: `emoji`, `title`, `description`, optional `action`                                               |
| `TagInput.tsx`        | Tag editor: `label`, `value`, `onChange`, optional `placeholder`                                                       |
| `StarRating.tsx`      | Star rating display and input                                                                                          |
| `IngredientInput.tsx` | Ingredient list editor shared by the food and drink recipe forms                                                       |
| `DrinkRecipeForm.tsx` | Drink recipe form; its types live in `src/types/drinkRecipe.ts` to satisfy `react-refresh/only-export-components`      |

## Tests and lint

- Vitest with React Testing Library; mock API calls.
- **Vitest doesn't run `tsc`.** Type errors in test code pass silently in `bun run test`. Run `bunx tsc --noEmit` (the /verify skill does) or `bun run build`, which chains `tsc`. When widening a shared type, sweep `src/test/factories.ts`.
- Format with `dprint fmt` (`dprint check` in CI); lint with `bun run lint` (`bun run lint:fix` to fix).
