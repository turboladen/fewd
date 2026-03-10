# Meal Planner Requirements

## Overview

A family meal planning application for a household of 4 people. The app helps plan meals for the week, manage recipes, and generate shopping lists. Built as a web application with a Rust/Axum backend and React frontend, using a local SQLite database.

## Core Assumptions

- **Single Family**: App serves one family, all persons belong to that family
- **Local Storage**: SQLite database stored locally, accessed from same computer
- **No Multi-tenancy**: No user accounts or authentication needed for MVP
- **Web App**: Rust/Axum backend + React frontend (accessed via browser)

## Future Features (Parking Lot)

📝 Guest support - temporary people for special occasions\
📝 Allergy/Intolerance tracking - medical restrictions\
📝 Historical dietary changes - track when goals/diets changed\
📝 Eating analysis - pattern analysis, nutrition trends\
📝 AI recipe generation - create recipes with AI\
📝 AI meal planning - auto-generate weekly plans\
📝 Multi-device sync - sync data across devices

---

## Entity Specifications

### Person Entity

Represents a family member who eats meals.

**Core Fields:**

**Identity**

- `id`: String (UUID)
- `name`: String - display name
- `birthdate`: Date - for age calculation
- `created_at`: DateTime
- `updated_at`: DateTime

**Dietary Information**

- `dietary_goals`: Optional String - freeform goals/macros (e.g., “2400 calories, 190g protein daily”)
- `dislikes`: Array of Strings - foods to avoid
- `favorites`: Array of Strings - foods they love
- `notes`: Optional String - any other dietary context

**Status**

- `is_active`: Boolean - currently eating with family (false if traveling)

**Example:**

```json
{
  "id": "person-steve",
  "name": "Steve",
  "birthdate": "1978-01-15",
  "dietary_goals": "2400 calories, 190g protein per day",
  "dislikes": ["mushrooms"],
  "favorites": ["tonkotsu ramen", "grilled chicken", "homemade pasta"],
  "notes": "Prefers authentic techniques, willing to do multi-day prep",
  "is_active": true,
  "created_at": "2026-01-16T10:00:00Z",
  "updated_at": "2026-01-16T10:00:00Z"
}
```

---

### Recipe Entity

Represents a reusable recipe with ingredients and instructions.

**Core Fields:**

**Identity**

- `id`: String (UUID)
- `name`: String
- `description`: Optional String - brief overview
- `source`: String - how created: “manual”, “markdown_import”, “ai_generated”
- `parent_recipe_id`: Optional String (UUID) - if adapted from another recipe
- `created_at`: DateTime
- `updated_at`: DateTime

**Recipe Details**

- `prep_time`: Optional Object - `{value: Integer, unit: "minutes"|"hours"|"days"}`
- `cook_time`: Optional Object - `{value: Integer, unit: "minutes"|"hours"|"days"}`
- `total_time`: Optional Object - `{value: Integer, unit: "minutes"|"hours"|"days"}`
- `servings`: Integer - base serving count
- `portion_size`: Optional Object - `{value: Number, unit: String}` (e.g., `{value: 60, unit: "grams"}`)
- `instructions`: String - step-by-step directions

**Ingredients**
Array of Ingredient objects:

```json
{
  "name": "flour",
  "amount": {
    "type": "single" | "range",
    "value": 2,      // if type is "single"
    "min": 1,        // if type is "range"
    "max": 2         // if type is "range"
  },
  "unit": "cups",
  "notes": "all-purpose"
}
```

**Amount Examples:**

- “2 cups” → `{type: "single", value: 2}, unit: "cups"`
- “1-2 cups” → `{type: "range", min: 1, max: 2}, unit: "cups"`
- “to taste” → `{type: "single", value: 1}, unit: "to taste"`
- “a pinch” → `{type: "single", value: 1}, unit: "pinch"`

**Nutrition** (optional)

```json
{
  "calories": 350,
  "protein_grams": 45,
  "carbs_grams": 10,
  "fat_grams": 15,
  "notes": "Approximate"
}
```

**Metadata & Usage**

- `tags`: Array of Strings (e.g., [“dinner”, “high-protein”, “quick”])
- `notes`: Optional String
- `icon`: Optional String - emoji (e.g., “🍗”, “🍝”)
- `is_favorite`: Boolean
- `times_made`: Integer - auto-increments when used in a meal
- `last_made`: Optional DateTime - auto-updates when used

**Example:**

```json
{
  "id": "recipe-grilled-chicken",
  "name": "Grilled Chicken with Vegetables",
  "description": "Simple grilled chicken with seasonal veggies",
  "source": "manual",
  "parent_recipe_id": null,
  "prep_time": { "value": 15, "unit": "minutes" },
  "cook_time": { "value": 30, "unit": "minutes" },
  "total_time": { "value": 45, "unit": "minutes" },
  "servings": 4,
  "portion_size": { "value": 1, "unit": "chicken breast" },
  "instructions": "1. Season chicken...\n2. Grill for 6-7 min per side...",
  "ingredients": [
    {
      "name": "chicken breast",
      "amount": { "type": "single", "value": 4 },
      "unit": "whole",
      "notes": "boneless, skinless"
    },
    {
      "name": "olive oil",
      "amount": { "type": "range", "min": 2, "max": 3 },
      "unit": "tbsp",
      "notes": ""
    },
    {
      "name": "salt",
      "amount": { "type": "single", "value": 1 },
      "unit": "to taste",
      "notes": ""
    }
  ],
  "nutrition_per_serving": {
    "calories": 350,
    "protein_grams": 45,
    "carbs_grams": 10,
    "fat_grams": 15
  },
  "tags": ["dinner", "high-protein", "quick"],
  "notes": "",
  "icon": "🍗",
  "is_favorite": true,
  "times_made": 12,
  "last_made": "2026-01-10T18:30:00Z",
  "created_at": "2025-12-01T10:00:00Z",
  "updated_at": "2026-01-10T18:30:00Z"
}
```

---

### Meal Entity

Represents a meal on a specific date with food assignments per person.

**Core Fields:**

**Identity**

- `id`: String (UUID)
- `date`: Date - which day this meal is for
- `meal_type`: String - “Breakfast”, “Lunch”, “Dinner”, or custom like “Snack”, “Dessert”
- `order`: Integer - for sorting meals within a day (0=Breakfast, 1=Lunch, 2=Dinner, 3+=custom)
- `created_at`: DateTime
- `updated_at`: DateTime

**Servings** - Array of PersonServing objects

Each person eating this meal has one PersonServing, which can be either:

**Recipe-Based Serving:**

```json
{
  "person_id": "person-steve",
  "food_type": "recipe",
  "recipe_id": "recipe-grilled-chicken",
  "servings_count": 0.5, // scaled from recipe's base servings
  "notes": "Extra spicy" // optional
}
```

**Ad-hoc Serving:**

```json
{
  "person_id": "person-viv",
  "food_type": "adhoc",
  "adhoc_items": [
    {
      "name": "hot dog",
      "amount": { "type": "single", "value": 1 },
      "unit": "whole",
      "notes": ""
    },
    {
      "name": "blueberries",
      "amount": { "type": "range", "min": 0.5, "max": 1 },
      "unit": "cup",
      "notes": ""
    }
  ],
  "notes": null
}
```

**Example Meal:**

```json
{
  "id": "meal-monday-dinner",
  "date": "2026-01-20",
  "meal_type": "Dinner",
  "order": 2,
  "servings": [
    {
      "person_id": "person-steve",
      "food_type": "recipe",
      "recipe_id": "recipe-grilled-chicken",
      "servings_count": 0.5,
      "notes": null
    },
    {
      "person_id": "person-amanda",
      "food_type": "recipe",
      "recipe_id": "recipe-grilled-chicken",
      "servings_count": 0.5,
      "notes": null
    },
    {
      "person_id": "person-viv",
      "food_type": "adhoc",
      "adhoc_items": [
        {
          "name": "hot dog",
          "amount": { "type": "single", "value": 1 },
          "unit": "whole",
          "notes": ""
        },
        {
          "name": "blueberries",
          "amount": { "type": "single", "value": 1 },
          "unit": "cup",
          "notes": ""
        },
        {
          "name": "chips",
          "amount": { "type": "single", "value": 1 },
          "unit": "handful",
          "notes": ""
        }
      ],
      "notes": null
    }
  ],
  "created_at": "2026-01-15T10:00:00Z",
  "updated_at": "2026-01-15T10:00:00Z"
}
```

**Business Rules:**

1. Each person can only appear once per meal
1. `servings_count` can be any positive number (0.5, 1, 2.5, etc.)
1. If `food_type` is “recipe”, `recipe_id` and `servings_count` are required
1. If `food_type` is “adhoc”, `adhoc_items` array is required
1. Default meals: Breakfast (order=0), Lunch (order=1), Dinner (order=2)
1. Custom meals have order >= 3
1. **When a recipe is used in a meal, increment that recipe’s `times_made` counter and update `last_made`**

**Recipe Scaling Example:**

- Recipe has `servings: 4` and ingredient “4 whole chicken breast”
- PersonServing has `servings_count: 0.5` (2 people eating)
- Scaled ingredient: “2 whole chicken breast”

---

### Shopping List (Computed View)

Not a stored entity - computed on-the-fly from meals in a date range.

**Input:**

- `start_date`: Date
- `end_date`: Date

**Output:** Array of AggregatedIngredient objects

```json
{
  "ingredient_name": "flour",
  "total_amount": {
    "type": "single" | "range",
    "value": 3.5,        // if type is "single"
    "min": 3,            // if type is "range"
    "max": 5             // if type is "range"
  },
  "total_unit": "cups",
  "items": [
    {
      "amount": {"type": "single", "value": 2},
      "unit": "cups",
      "source_type": "recipe",
      "source_id": "recipe-bread",
      "meal_id": "meal-monday-breakfast"
    },
    {
      "amount": {"type": "range", "min": 1, "max": 3},
      "unit": "cups",
      "source_type": "recipe",
      "source_id": "recipe-cake",
      "meal_id": "meal-tuesday-lunch"
    }
  ]
}
```

**Business Logic:**

1. **Collect ingredients:**

- Query all Meals between start_date and end_date
- For each PersonServing:
  - If `food_type` is “recipe”: Get Recipe, scale ingredients by `servings_count`
  - If `food_type` is “adhoc”: Use `adhoc_items` directly

1. **Group by ingredient name:**

- Case-insensitive grouping (e.g., “Chicken Breast” = “chicken breast”)
- Exact string matching for MVP (no fuzzy matching)

1. **Unit conversion:**

- Use unit conversion library (`uom` crate in Rust)
- Convert to common base units:
  - **Weight**: grams (g, kg, oz, lb, mg)
  - **Volume**: ml (ml, L, cup, tbsp, tsp, fl oz, pint, quart, gallon)
  - **Count/Discrete**: no conversion (whole, piece, clove, can, etc.)

1. **Sum amounts:**

- **Single values:** Convert to common unit, sum, display in most common or largest appropriate unit
- **Ranges:** Sum min values and max values separately (e.g., 1-2 cups + 2-3 cups = 3-5 cups)
- **Mixed single + range:** For MVP, don’t auto-sum (future feature)
- **Non-convertible units:** Group under ingredient name but don’t sum

1. **Display format:**

- Decimals (e.g., “2.5 cups” not “2 1/2 cups”)
- Most common unit from source items, or largest appropriate unit

**Ingredient Normalization:**

For MVP:

- Case-insensitive grouping
- Exact string matching (no fuzzy matching)
- Manual merge UI for combining similar ingredients

Future: Fuzzy matching to suggest merges

**Example Display:**

```
Flour
  - 2 cups (Bread recipe for Monday Breakfast)
  - 1-2 cups (Cake recipe for Tuesday Lunch)
  Total: 3-4 cups

Chicken Breast
  - 4 whole (Grilled Chicken for Monday Dinner)
  - 2 whole (Chicken Soup for Tuesday Lunch)
  Total: 6 whole

Salt
  - 1 tsp (Soup recipe)
  - to taste (Pasta sauce recipe)
  (Items listed separately - different unit types)
```

---

## User Workflows

### Recipe Management

**Add Recipe Manually:**

1. Click “Add Recipe”
1. Fill in name, description, prep/cook times, servings
1. Add ingredients one by one (amount, unit, name, notes)
1. Write instructions
1. Optionally add tags, nutrition info, icon
1. Save

**Import Recipe from Markdown:**

1. Click “Import Markdown”
1. Paste markdown in format:

```markdown
# Recipe Name

Description
Prep time: 30 min
Servings: 4

## Ingredients

- 2 cups flour
- 1 tsp salt

## Instructions

1. Mix ingredients...
2. Bake...
```

1. Review parsed recipe
1. Edit if needed
1. Save

**Edit Recipe:**

1. Find recipe in list
1. Click edit
1. Modify any fields
1. Save

**Delete Recipe:**

1. Find recipe
1. Click delete
1. Confirm (warning if recipe is used in future meals)

**View Recipe:**

- See full details including ingredients, instructions, nutrition
- See how many times made and when last made
- See if it’s a favorite

### Meal Planning

**View Weekly Calendar:**

- See current week (Monday-Sunday)
- Each day shows Breakfast, Lunch, Dinner (+ any custom meals)
- Navigate to previous/next week
- Today is highlighted

**Plan a Meal:**

1. Click on meal slot (e.g., “Monday Dinner”)
1. Modal opens showing all family members
1. For each person:

- **Option A:** Select recipe from dropdown, specify serving count
- **Option B:** Add ad-hoc items (like “1 hot dog, 1 cup blueberries”)
- **Option C:** Leave blank (not eating this meal)

1. Save meal

**Add Custom Meal:**

1. Click “+ Add Meal” on a day
1. Enter meal type (e.g., “Snack”, “Dessert”)
1. Plan as normal

**Edit Meal:**

1. Click existing meal
1. Modify person assignments
1. Save

**Delete Meal:**

1. Click meal
1. Delete button
1. Confirm

### Shopping List

**Generate Shopping List:**

1. Go to Shopping List view
1. Select date range (defaults to next 7 days)
1. See aggregated ingredient list with totals
1. Each ingredient shows:

- Total amount needed
- Breakdown by source (which recipes/meals)

1. Copy to iOS Reminders or other list app manually

---

## Technical Requirements

### Tech Stack

**Backend:**

- Language: Rust
- Framework: Axum
- ORM: SeaORM 1.1
- Database: SQLite (local file)
- Unit Conversion: `uom` crate

**Frontend:**

- Framework: React 18 + TypeScript
- Build Tool: Vite
- Data Fetching: TanStack Query (React Query)
- Styling: Tailwind CSS
- Icons: Heroicons (custom SVG components)

**Deployment:**

- Standalone Axum server serving both API and static frontend
- Supported platforms: macOS, Linux (wherever Rust compiles)

### Database Schema

**people table:**

```sql
CREATE TABLE people (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    birthdate DATE NOT NULL,
    dietary_goals TEXT,
    dislikes TEXT, -- JSON array
    favorites TEXT, -- JSON array
    notes TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
```

**recipes table:**

```sql
CREATE TABLE recipes (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    source TEXT NOT NULL,
    parent_recipe_id TEXT,
    prep_time TEXT, -- JSON: {value, unit}
    cook_time TEXT, -- JSON: {value, unit}
    total_time TEXT, -- JSON: {value, unit}
    servings INTEGER NOT NULL,
    portion_size TEXT, -- JSON: {value, unit}
    instructions TEXT NOT NULL,
    ingredients TEXT NOT NULL, -- JSON array
    nutrition_per_serving TEXT, -- JSON object
    tags TEXT, -- JSON array
    notes TEXT,
    icon TEXT,
    is_favorite BOOLEAN NOT NULL DEFAULT 0,
    times_made INTEGER NOT NULL DEFAULT 0,
    last_made TIMESTAMP,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    FOREIGN KEY (parent_recipe_id) REFERENCES recipes(id)
);
```

**meals table:**

```sql
CREATE TABLE meals (
    id TEXT PRIMARY KEY,
    date DATE NOT NULL,
    meal_type TEXT NOT NULL,
    order_index INTEGER NOT NULL,
    servings TEXT NOT NULL, -- JSON array of PersonServing objects
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
```

### Non-Functional Requirements

**Performance:**

- App should feel instant for typical operations
- Shopping list generation for 1 week should be < 1 second

**Usability:**

- Clean, simple UI
- Minimal clicks to accomplish tasks
- Clear error messages
- Loading indicators for async operations

**Data Storage:**

- SQLite database in OS-appropriate app data directory
- Database file persists between app launches
- No cloud sync for MVP

**Reliability:**

- No data loss
- Graceful error handling
- Transaction safety for database operations

---

## Success Criteria

MVP is successful when:

1. ✅ Can manage family members (4 people)
1. ✅ Can create and manage recipes
1. ✅ Can import recipes from markdown
1. ✅ Can plan meals for a week
1. ✅ Can assign different recipes to different people
1. ✅ Can use ad-hoc food items (not recipes)
1. ✅ Can generate shopping list for date range
1. ✅ Shopping list correctly aggregates and converts units
1. ✅ App is stable and usable daily
1. ✅ Data persists correctly

---

## Out of Scope for MVP

- ❌ AI recipe generation
- ❌ AI meal planning suggestions
- ❌ Multi-device sync
- ❌ User accounts / authentication
- ❌ Allergy/intolerance warnings
- ❌ Nutrition tracking over time
- ❌ Recipe scaling UI (manual calculation for now)
- ❌ Print shopping list
- ❌ Recipe photos
- ❌ Recipe ratings/reviews per person
- ❌ Meal history / analytics
- ❌ Guest support
- ❌ Fuzzy ingredient matching
- ❌ Ingredient inventory tracking (what you have at home)
