# Meal Planner Implementation Plan

## Completed Phases (MVP)

| Phase   | Description                                           | Status |
| ------- | ----------------------------------------------------- | ------ |
| Phase 0 | Project Setup & Scaffolding                           | Done   |
| Phase 1 | Person Entity (CRUD)                                  | Done   |
| Phase 2 | Recipe Entity (CRUD + Markdown Import)                | Done   |
| Phase 3 | Meal Entity & Planning (Weekly Calendar)              | Done   |
| Phase 4 | Shopping List (Aggregation + Unit Conversion)         | Done   |
| Phase 5 | Polish & Initial Data (Seed Data, UX, Error Handling) | Done   |

---

## Phase 6: Recipe Rating

**Goal:** Add a 5-star rating system to recipes for use in future suggestion features.

**Why first:** Small, self-contained change that lays groundwork for meal suggestions. No dependencies on other new features.

### Tasks:

**6.1: Add Rating to Recipe Entity**

- Add `rating: Option<f64>` field to `recipe` entity (nullable, whole numbers 1-5)
- Create migration: `m20260208_000004_add_recipe_rating.rs`
- Update `RecipeService` and `UpdateRecipeDto` to support setting the rating

**6.2: Add Rating to Recipe Detail UI**

- Add star rating display on recipe detail view (RecipeDetail component)
- Clickable stars (whole numbers, hover preview)
- Show on recipe list cards as well (small, non-interactive display)
- Add `useUpdateRecipe` call on rating change

**6.3: Add Tests**

- Rust: test rating persistence (create recipe, set rating, verify)
- Rust: test rating validation (rejects 0, 5.5, fractional values)
- Frontend: test rating passthrough in parseRecipe

**Verify Phase 6:**

- [x] Can rate a recipe from the detail view
- [x] Rating displays on recipe cards in list
- [x] Rating persists after app restart
- [x] Whole number ratings 1-5 with hover preview

**Deliverable:** Recipe rating system

---

## Phase 7: Recipe Scaling

**Goal:** Scale a recipe to a different number of servings, with smart handling of fractional/indivisible ingredients.

**Why now:** Pure math feature, no AI needed. Uses existing `parent_recipe_id` field. Builds on unit converter from Phase 4.

### Tasks:

**7.1: Create Scaling Service**

- File: `src-tauri/src/services/recipe_scaler.rs`
- `scale_recipe(recipe, new_servings) -> CreateRecipeDto` — multiply all ingredient amounts by `new_servings / recipe.servings`
- Flag indivisible ingredients: identify items with "whole", "piece", etc. units where the scaled amount isn't a whole number
- Return scaled recipe DTO + list of flagged ingredients with their fractional values

**7.2: Create Scaling Command**

- `scale_recipe(id, new_servings, mode)` where mode is `"create_new"` or `"modify_in_place"`
- For `create_new`: create recipe with `parent_recipe_id` set to original, source = "scaled", name = "Original Name (scaled to X servings)"
- For `modify_in_place`: update the existing recipe's ingredients and servings count

**7.3: Scaling UI**

- Add "Scale Recipe" button on recipe detail view
- Opens a modal/panel:
  - Number input for target servings
  - Preview of scaled ingredients
  - Visually flag fractional indivisible ingredients (amber highlight) with the computed value
  - User can manually adjust flagged amounts before saving
  - Radio: "Save as new recipe" vs "Update this recipe"
- Save button creates/updates recipe

**7.4: Add Tests**

- Rust: test scaling math, indivisible detection, both modes (create_new, modify_in_place)
- Rust: test parent_recipe_id is set correctly for derived recipes
- Frontend: test scaling modal, preview, flagged ingredients display

**Verify Phase 7:**

- [x] Can scale a 4-serving recipe to 6 servings
- [x] Ingredient amounts are multiplied correctly
- [x] Fractional "whole" items (e.g., 2.25 eggs) are flagged in the UI
- [x] User can adjust flagged amounts before saving
- [x] "Save as new" creates a derived recipe with parent_recipe_id
- [x] "Update this recipe" modifies in place
- [x] Derived recipes show link to parent in detail view

**Deliverable:** Recipe scaling with smart fractional ingredient handling

---

## Phase 8: Inline Ingredient Enhancement (Caroline Chambers Style)

**Goal:** View mode that injects ingredient amounts into instruction steps, so you don't have to reference the ingredient list while cooking.

### Tasks:

**8.1: Create Enhancement Algorithm**

- File: `src-tauri/src/services/recipe_enhancer.rs`
- `enhance_instructions(ingredients, instructions) -> String`
- For each instruction step, scan for ingredient names (case-insensitive)
- Rules:
  - If step already has a number before the ingredient name, leave it alone
  - If ingredient appears in only one step, inject full amount (e.g., "Add flour" becomes "Add **2 cups flour**")
  - If ingredient appears in multiple steps with no amounts, inject full amount on first occurrence, leave subsequent as-is
- Bold formatting (`**amount unit name**`) for injected amounts

**8.2: Create Tauri Command**

- `enhance_recipe_instructions(id) -> String` — returns enhanced instruction text

**8.3: Enhanced View Toggle**

- Add "Enhanced view" toggle button on recipe detail's instructions section
- When toggled on, fetch enhanced instructions and display them (with bold amounts)
- Toggle off returns to original instructions
- Optionally: "Save enhanced version" button that overwrites instructions field

**8.4: Add Tests**

- Rust: test injection for single-step ingredient, multi-step ingredient, already-numbered step
- Rust: test bold formatting
- Frontend: test toggle behavior

**Verify Phase 8:**

- [x] Toggle shows enhanced instructions with inline amounts
- [x] Already-numbered ingredients are left alone
- [x] Multi-step ingredients get amount on first occurrence
- [x] Toggle off returns to original text
- [x] Bold formatting is visible and readable

**Deliverable:** Caroline Chambers-style enhanced recipe view

---

## Phase 9: Meal Templates

**Goal:** Save and reuse common meal configurations to speed up weekly planning.

### Tasks:

**9.1: Create Template Entity**

- Migration: `m20260208_000005_create_meal_templates.rs`
- Fields: `id`, `name`, `meal_type` (breakfast/lunch/dinner/snack), `servings` (JSON, same PersonServing format as meals), `created_at`, `updated_at`
- Entity: `src-tauri/src/entities/meal_template.rs`

**9.2: Template Service + Commands**

- CRUD service: `src-tauri/src/services/meal_template_service.rs`
- Commands: `get_all_meal_templates`, `create_meal_template`, `update_meal_template`, `delete_meal_template`
- "Save as template" command: takes an existing meal ID, creates a template from it

**9.3: Template Selector UI**

- When editing a meal slot, add "Use Template" button
- Opens template picker, sorted/filtered by meal type (shows relevant type first)
- Selecting a template populates the meal editor's servings for matching people
- Only fills in people defined in the template; leaves others untouched

**9.4: Template Management UI**

- Accessible from a settings/manage area or a tab
- List all templates with meal type and assigned people
- Edit template name, delete template
- "Save as template" button on existing planned meals (in meal editor)

**9.5: Add Tests**

- Rust: CRUD tests for template service
- Rust: test "save from meal" creates correct template
- Frontend: test template picker filtering, applying template to meal editor

**Verify Phase 9:**

- [x] Can save an existing meal as a template
- [x] Can browse templates filtered by meal type
- [x] Applying a template fills in the correct people with correct food
- [x] Template only affects the people it's defined for
- [x] Can edit and delete templates
- [x] Templates persist across app restarts

**Deliverable:** Reusable meal templates

---

## Phase 10: Deterministic Meal Suggestions

**Goal:** Suggest meals based on historical data — no AI needed. Three categories: recent favorites, forgotten hits, and untried recipes.

**Why before AI:** These suggestions are fast, reliable, and provide immediate value. They also establish the suggestion UX patterns that AI suggestions will reuse.

### Tasks:

**10.1: Create Suggestion Service**

- File: `src-tauri/src/services/suggestion_service.rs`
- `get_suggestions(db, person_ids, meal_type, date_range) -> MealSuggestions`
- Three categories:
  - **Recent favorites:** Recipes most used in the past 2 weeks for the selected people
  - **Forgotten hits:** Recipes with high `times_made` + good `rating` but no meals in last 30+ days
  - **Untried in system:** Recipes in the database that have never been assigned to the selected people (cross-reference meals history)
- Each category returns up to 5 suggestions with recipe info + relevance reason

**10.2: Suggestion Command**

- `get_meal_suggestions(person_ids, meal_type, lookback_days)` — returns structured suggestions

**10.3: Suggestion UI (Meal Planner Integration)**

- Add "Suggest" button in the meal editor (next to people list)
- Opens suggestion panel:
  - Checkboxes to select which family members to suggest for
  - Three collapsible sections: "Recent Favorites", "Used to Love", "Something Different"
  - Each suggestion shows recipe name, rating, last made date
  - Click a suggestion to assign it to the selected people in the meal editor
- After deterministic suggestions, show a "Want AI suggestions?" prompt (disabled until Phase 12)

**10.4: Add Tests**

- Rust: test each suggestion category with seeded data
- Rust: test person filtering
- Frontend: test suggestion panel rendering, selection flow

**Verify Phase 10:**

- [x] "Recent favorites" shows most-used recipes from past 2 weeks
- [x] "Forgotten hits" shows well-rated recipes not used recently
- [x] "Something different" shows recipes never assigned to selected people
- [x] Clicking a suggestion populates the meal editor
- [x] Suggestions respect person selection (only consider selected people's history)

**Deliverable:** Data-driven meal suggestion engine

---

## Phase 11: AI Infrastructure (Claude API Integration)

**Goal:** Set up the foundation for AI features — API key management, prompt construction, and a draft/review UX pattern.

**Why separate phase:** All AI features (recipe adaptation, AI suggestions) share this infrastructure. Building it once prevents duplication.

### Tasks:

**11.1: Settings Entity + UI**

- Migration: `m20260208_000006_create_settings.rs` (key-value table: `key TEXT PRIMARY KEY, value TEXT`)
- Service: `src-tauri/src/services/settings_service.rs` — `get(key)`, `set(key, value)`
- Commands: `get_setting`, `set_setting`
- Settings page/modal accessible from app header
- API key input (Anthropic API key), stored locally, masked display
- Model selector dropdown (choose which Claude model to use for AI features)
- Test connection button (makes a minimal API call to verify key works with selected model)

**11.2: Claude API Client**

- Add `reqwest` dependency (HTTP client)
- File: `src-tauri/src/services/claude_client.rs`
- `send_message(api_key, model, system_prompt, user_message) -> Result<String, Error>`
- Handles Anthropic Messages API (latest models as of Feb 2026)
- Model selection: user picks from available models in Settings (stored as a setting)
- Available models: Claude Sonnet 4 (`claude-sonnet-4-20250514`), Claude Sonnet 4.5 (`claude-sonnet-4-5-20241022`), Claude Opus 4 (`claude-opus-4-20250514`), Claude Opus 4.6 (when available)
- Default to Claude Sonnet 4 (best balance of cost/quality for recipe tasks)
- Error handling for rate limits, invalid key, network issues

**11.3: Draft/Review UX Pattern**

- Shared component: `DraftReview` — displays AI-generated content with "Accept", "Edit", "Reject" actions
- On accept: saves the result (recipe creation, meal assignment, etc.)
- On edit: opens editable form pre-filled with AI output
- On reject: discards and optionally asks for regeneration
- Loading state with cancel support

**11.4: Prompt Builder Utility**

- File: `src-tauri/src/services/prompt_builder.rs`
- `build_person_context(people) -> String` — formats person profiles for prompts
- `build_recipe_context(recipe) -> String` — formats recipe details
- `build_meal_history_context(meals, recipes) -> String` — recent meal summary
- Keeps prompt size manageable (truncate history if too long)

**11.5: Add Tests**

- Rust: test prompt builder output formatting
- Rust: test settings CRUD
- Frontend: test DraftReview component states (loading, reviewing, accepted)
- Note: Claude API client tests use mocked responses (no real API calls in tests)

**Verify Phase 11:**

- [x] Can set/get Anthropic API key in settings
- [x] Can select which Claude model to use from a dropdown
- [x] API key is stored locally and displays masked
- [x] Test connection works with valid key and selected model
- [x] DraftReview component shows accept/edit/reject flow
- [x] Prompt builder produces well-formatted context strings

**Deliverable:** AI integration infrastructure ready for feature development

---

## Phase 12: AI Recipe Adaptation

**Goal:** Generate new recipes from existing ones, adapted for specific people's dietary goals, preferences, and restrictions. Also adapt for specific diet types.

### Tasks:

**12.1: Recipe Adaptation Service**

- File: `src-tauri/src/services/recipe_adapter.rs`
- `adapt_recipe(api_key, recipe, people, user_instructions) -> Result<CreateRecipeDto, Error>`
- Constructs prompt with:
  - Original recipe (full details)
  - Selected people's profiles (dietary goals, favorites, dislikes)
  - User's free-text instructions (e.g., "make this keto", "adapt for the kids")
- Parses Claude's response into a `CreateRecipeDto` (structured JSON output)
- Sets `parent_recipe_id` to original recipe, source = "ai_adapted"

**12.2: Adaptation UI**

- "Adapt Recipe" button on recipe detail view
- Opens adaptation panel:
  - Checkboxes for family members to optimize for
  - For each selected person, show their profile info with checkboxes to include/exclude specific fields (dietary goals, dislikes, etc.)
  - Free-text input for additional instructions
  - "Generate" button
- Shows DraftReview with the generated recipe
- On accept: creates new recipe in database with parent link

**12.3: Add Tests**

- Rust: test prompt construction (verify person context, recipe context included)
- Rust: test response parsing (mock AI response → CreateRecipeDto)
- Frontend: test adaptation panel, person selection, field toggles

**Verify Phase 12:**

- [x] Can select people to adapt a recipe for
- [x] Can toggle which profile fields are included per person
- [x] Can add free-text instructions
- [x] AI generates an adapted recipe (shown in draft review)
- [x] Accepted recipe is saved with parent_recipe_id link
- [x] Adapted recipe appears in recipe list

**Deliverable:** AI-powered recipe adaptation

---

## Phase 13: AI Meal Suggestions

**Goal:** When deterministic suggestions aren't enough, generate new meal ideas using Claude, personalized to family members.

### Tasks:

**13.1: AI Suggestion Service**

- File: `src-tauri/src/services/ai_suggestion_service.rs`
- `suggest_meals(api_key, people, meal_type, context) -> Result<Vec<MealSuggestion>, Error>`
- Context includes:
  - Selected people's profiles
  - Recent meal history (what they've been eating)
  - Meal type (breakfast/lunch/dinner)
  - User preferences: balanced vs. indulgent, any specific requests
- Returns structured suggestions with recipe name, description, and ingredients
- Each suggestion can be converted to a `CreateRecipeDto` for saving

**13.2: AI Suggestion UI (extends Phase 10 UI)**

- The "Want AI suggestions?" prompt from Phase 10 becomes active
- Wizard flow:
  1. Select family members (checkboxes)
  2. For each person, toggle which profile fields to include
  3. Choose meal character: "Balanced" (default), "Indulgent / treat", "Quick & easy", or custom text
  4. "Generate Suggestions" button
- Shows 3-5 AI-generated meal ideas
- Each idea has: name, short description, key ingredients
- Click to expand: full recipe draft
- "Use this" button saves recipe to database + assigns to meal slot
- "None of these" option to regenerate with feedback

**13.3: Recipe Tag Auto-Suggestion**

- When AI creates a recipe (from adaptation or suggestion), auto-suggest tags based on content
- Tags like: `high-protein`, `veggie`, `quick`, `kid-friendly`, `indulgent`
- User can accept/modify suggested tags before saving

**13.4: Add Tests**

- Rust: test prompt construction for suggestions
- Rust: test response parsing (mock response → suggestions)
- Frontend: test wizard flow, suggestion cards, save flow

**Verify Phase 13:**

- [x] Deterministic suggestions show first, AI option follows
- [x] Can select people and toggle profile fields
- [x] Can choose meal character (balanced, indulgent, etc.)
- [x] AI generates 3-5 contextual meal suggestions
- [x] Each suggestion can be saved as a recipe and assigned to the meal
- [x] Tags are auto-suggested on AI-created recipes

**Deliverable:** Full AI-powered meal suggestion engine

---

## Phase 14: Database Configuration

**Goal:** Allow configuring the database location via the Settings UI.

### Tasks:

**14.1: Configurable Database Location**

- Add `GET /settings/db-location` and `PUT /settings/db-location` routes
- Read current `DATABASE_PATH` from environment/config, expose via API
- Support setting a new path, validate it's writable, copy existing data if needed
- Server restart required after changing location

**14.2: Database Location Settings UI**

- Add "Database Location" section to `SettingsPanel.tsx`
- Text input showing current path with a "Browse" or manual edit option
- Validation feedback (writable, existing DB detection)
- "Copy data to new location" option when changing paths
- Restart notice after changing location

**Verify Phase 14:**

- [x] Default DB location unchanged when no config exists
- [x] Can view current DB location in Settings
- [ ] Can change DB location via Settings
- [ ] Data is copied to new location when requested
- [ ] CI passes (fmt, clippy, test, dprint, lint, bun test)

**Deliverable:** Configurable database location via Settings UI

---

## Phase 15: Favicon & Branding

**Goal:** Add a custom favicon and browser tab icon for the web app.

### Tasks:

**15.1: Create Icon Source**

- Design a programmatic SVG: fork and knife crossed over a circular plate, sage green background
- Simple geometric shapes that read well at 16x16 and 32x32

**15.2: Generate Favicon Files**

- Create `favicon.svg` (scalable, used by modern browsers)
- Generate `favicon.ico` (16x16 + 32x32 for legacy browsers)
- Generate `apple-touch-icon.png` (180x180 for iOS home screen)
- Add to `public/` directory and reference in `index.html`

**Verify Phase 15:**

- [ ] Browser tab shows custom favicon
- [ ] iOS "Add to Home Screen" shows the icon
- [ ] SVG favicon matches the app's color palette

**Deliverable:** Custom favicon and browser branding

---

## Phase 16: AI Recipe Import (URL + PDF)

**Goal:** Add URL and PDF import methods alongside existing markdown import, using AI to extract recipes from unstructured content.

### Tasks:

**16.1: Backend — New Service**

- Add `pdf-extract` and `html2text` crate dependencies
- Create `recipe_import_service.rs` with `import_from_url()` and `import_from_pdf()`
- URL import: fetch HTML via reqwest, try JSON-LD extraction first, fall back to html2text, send to Claude
- PDF import: extract text via pdf-extract, send to Claude
- Claude system prompt: "Extract the recipe and return JSON matching CreateRecipeDto schema"
- Reuse `strip_code_fences` from recipe_adapter, extract shared JSON schema
- Error types: NetworkError, PdfError, ContentTooShort, AiError, ParseError

**16.2: Backend — New Commands**

- Add `import_recipe_from_url` command (fetch + AI extract + save + track tokens)
- Add `import_recipe_from_file` command (read PDF + AI extract + save + track tokens)
- Follow existing `adapt_recipe` pattern for API key/model/token tracking
- Register commands in main.rs

**16.3: Frontend — Import UI**

- Add `useImportRecipeFromUrl` and `useImportRecipeFromFile` mutation hooks
- Update `ImportRecipeForm` with three-tab UI: Markdown (existing), URL, PDF
- URL tab: text input + "Import" button with "Analyzing recipe with AI..." loading state
- PDF tab: file upload input with PDF filter + "Import" button
- Rename "Import Markdown" button to "Import"

**Verify Phase 16:**

- [ ] Paste a recipe URL → imports correctly with ingredients, instructions, etc.
- [ ] Pick a PDF recipe file → imports correctly
- [ ] Token usage increments in Settings
- [ ] Error handling: bad URL, empty PDF, no API key
- [ ] CI passes

**Deliverable:** AI-powered recipe import from URLs and PDFs

---

## Phase 17: Styling Facelift

**Goal:** Replace default Tailwind theme with a warm, food-themed personality. Custom color palette, heading font, subtle shadows.

### Tasks:

**17.1: Tailwind Theme**

- Define custom color scales in `tailwind.config.js`: primary (sage green), secondary (terracotta), accent (warm gold), surface (warm cream)
- Add heading font family (Georgia — system font, no import)

**17.2: Global Styles**

- Update `index.css`: warm background, heading font via `@layer base`

**17.3: Color Migration**

- Mechanical find-and-replace across all component files (~16):
  - `gray-*` → `stone-*` (~244 occurrences, Tailwind's built-in warm gray)
  - `blue-*` → `primary-*` (~40 occurrences)
  - `yellow-*` → `accent-*` (favorites/stars)
  - `teal-*` → `secondary-*` (adapt recipe)
  - `bg-gray-50` → `bg-surface` (page backgrounds)

**17.4: Nav & Card Refinement**

- Nav bar: warmer background, subtle shadow
- Cards/panels: add `shadow-sm` for depth

**Verify Phase 17:**

- [ ] Warm tones throughout, no stray blue/cold-gray remaining
- [ ] Heading font (Georgia) on all h1/h2/h3
- [ ] Cards have subtle shadows
- [ ] `dprint check && bun run lint && bun run test` all pass
- [ ] Build and visual inspection

**Deliverable:** Warm, food-themed visual design

---

## Task Breakdown Summary

Give these tasks to Claude Code in order:

1. **Phase 6** — Recipe rating (small, self-contained, foundational for suggestions)
2. **Phase 7** — Recipe scaling (pure math, no AI, uses existing parent_recipe_id)
3. **Phase 8** — Inline ingredient enhancement (deterministic text processing)
4. **Phase 9** — Meal templates (new entity, CRUD, planner integration)
5. **Phase 10** — Deterministic meal suggestions (data-driven, establishes suggestion UX)
6. **Phase 11** — AI infrastructure (API key, client, prompt builder, DraftReview pattern)
7. **Phase 12** — AI recipe adaptation (first AI feature, uses all Phase 11 infra)
8. **Phase 13** — AI meal suggestions (builds on Phases 10, 11, and 12)
9. **Phase 14** — Database configuration (configurable DB location via Settings UI)
10. **Phase 15** — Favicon & branding (custom favicon for the web app)
11. **Phase 16** — AI recipe import from URL and PDF (extends import system)
12. **Phase 17** — Styling facelift (warm color palette, heading font, polish)

---

## Architecture Notes

### Shared Infrastructure

These features share common patterns:

- **Recipe derivation:** Phases 7, 12, 13 all create new recipes from existing ones via `parent_recipe_id`
- **DraftReview UX:** Phases 12 and 13 both use the preview-before-save pattern
- **Person context:** Phases 10, 12, 13 all need person profile data for filtering/prompting
- **Meal history queries:** Phases 10 and 13 both analyze past meals

### Data Flow for AI Features

```
User selects people + preferences
    -> Prompt Builder constructs context
        -> Claude API generates response
            -> Response parser extracts structured data
                -> DraftReview shows preview
                    -> User accepts/edits/rejects
                        -> Service saves to database
```

### Recipe Lineage

```
Original Recipe (source: "manual" | "markdown_import" | "url_import" | "pdf_import")
    |-- Scaled Recipe (source: "scaled", parent_recipe_id: original)
    |-- AI Adapted Recipe (source: "ai_adapted", parent_recipe_id: original)
    +-- AI Suggested Recipe (source: "ai_suggested", parent_recipe_id: null)
```
