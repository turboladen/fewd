# Meal Planner Implementation Plan

## Phase 0: Project Setup & Scaffolding

**Goal:** Get Tauri + React + SeaORM project initialized and running

### Tasks:

**0.1: Initialize Tauri Project**

```bash
npm create tauri-app@latest
# Choose:
# - Project name: meal-planner
# - UI template: React + TypeScript
# - Package manager: npm (or pnpm/yarn if you prefer)
```

**0.2: Add Rust Dependencies**

Update `src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sea-orm = { version = "1.1", features = ["sqlx-sqlite", "runtime-tokio-native-tls", "macros"] }
tokio = { version = "1", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
```

Create migration crate at `src-tauri/migration/Cargo.toml`:

```toml
[package]
name = "migration"
version = "0.1.0"
edition = "2021"

[dependencies]
async-trait = "0.1"
sea-orm-migration = "1.1"

[lib]
name = "migration"
path = "src/lib.rs"
```

**0.3: Add Frontend Dependencies**

Update `package.json`:

```bash
npm install @tanstack/react-query @tanstack/react-router
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p
```

**0.4: Project Structure**

Create directory structure:

```
src-tauri/
├── src/
│   ├── commands/
│   │   └── mod.rs
│   ├── entities/
│   │   └── mod.rs
│   ├── services/
│   │   └── mod.rs
│   ├── db.rs
│   └── main.rs
├── migration/
│   └── src/
│       └── lib.rs
```

```
src/
├── components/
├── pages/
├── hooks/
├── types/
└── App.tsx
```

**0.5: Verify Build**

```bash
npm run tauri dev
```

Should see empty Tauri window with React app running.

**Deliverable:** Clean Tauri + React app that compiles and runs

-----

## Phase 1: Person Entity (CRUD)

**Goal:** Complete Person management - backend + frontend

### Backend Tasks:

**1.1: Create Person Migration**

File: `src-tauri/migration/src/m20260118_000001_create_people.rs`

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(People::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(People::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(People::Name).string().not_null())
                    .col(ColumnDef::new(People::Birthdate).date().not_null())
                    .col(ColumnDef::new(People::DietaryGoals).string())
                    .col(ColumnDef::new(People::Dislikes).text().not_null())
                    .col(ColumnDef::new(People::Favorites).text().not_null())
                    .col(ColumnDef::new(People::Notes).string())
                    .col(ColumnDef::new(People::IsActive).boolean().not_null().default(true))
                    .col(ColumnDef::new(People::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(People::UpdatedAt).timestamp_with_time_zone().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(People::Table).to_owned()).await
    }
}

#[derive(Iden)]
enum People {
    Table,
    Id,
    Name,
    Birthdate,
    DietaryGoals,
    Dislikes,
    Favorites,
    Notes,
    IsActive,
    CreatedAt,
    UpdatedAt,
}
```

Update `src-tauri/migration/src/lib.rs`:

```rust
pub use sea_orm_migration::prelude::*;

mod m20260118_000001_create_people;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260118_000001_create_people::Migration)]
    }
}
```

**1.2: Create Person Entity**

File: `src-tauri/src/entities/person.rs`

```rust
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "people")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub birthdate: Date,
    pub dietary_goals: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub dislikes: String, // JSON array
    #[sea_orm(column_type = "Text")]
    pub favorites: String, // JSON array
    pub notes: Option<String>,
    pub is_active: bool,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Helper methods for JSON fields
impl Model {
    pub fn get_dislikes(&self) -> Vec<String> {
        serde_json::from_str(&self.dislikes).unwrap_or_default()
    }

    pub fn get_favorites(&self) -> Vec<String> {
        serde_json::from_str(&self.favorites).unwrap_or_default()
    }
}
```

Update `src-tauri/src/entities/mod.rs`:

```rust
pub mod person;
```

**1.3: Create DTOs**

File: `src-tauri/src/commands/person.rs` (start with DTOs):

```rust
use serde::{Deserialize, Serialize};
use chrono::NaiveDate;

#[derive(Debug, Deserialize, Serialize)]
pub struct CreatePersonDto {
    pub name: String,
    pub birthdate: NaiveDate,
    pub dietary_goals: Option<String>,
    pub dislikes: Vec<String>,
    pub favorites: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePersonDto {
    pub name: Option<String>,
    pub birthdate: Option<NaiveDate>,
    pub dietary_goals: Option<String>,
    pub dislikes: Option<Vec<String>>,
    pub favorites: Option<Vec<String>>,
    pub notes: Option<String>,
    pub is_active: Option<bool>,
}
```

**1.4: Create Person Service**

File: `src-tauri/src/services/person_service.rs`

```rust
use sea_orm::*;
use crate::entities::person::{self, Entity as Person};
use crate::commands::person::{CreatePersonDto, UpdatePersonDto};

pub struct PersonService;

impl PersonService {
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<person::Model>, DbErr> {
        Person::find()
            .filter(person::Column::IsActive.eq(true))
            .order_by_asc(person::Column::Name)
            .all(db)
            .await
    }

    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: String,
    ) -> Result<Option<person::Model>, DbErr> {
        Person::find_by_id(id).one(db).await
    }

    pub async fn create(
        db: &DatabaseConnection,
        data: CreatePersonDto,
    ) -> Result<person::Model, DbErr> {
        let now = chrono::Utc::now();
        let person = person::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(data.name),
            birthdate: Set(data.birthdate),
            dietary_goals: Set(data.dietary_goals),
            dislikes: Set(serde_json::to_string(&data.dislikes).unwrap()),
            favorites: Set(serde_json::to_string(&data.favorites).unwrap()),
            notes: Set(data.notes),
            is_active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        };

        person.insert(db).await
    }

    pub async fn update(
        db: &DatabaseConnection,
        id: String,
        data: UpdatePersonDto,
    ) -> Result<person::Model, DbErr> {
        let person = Person::find_by_id(id.clone())
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Person not found".to_string()))?;

        let mut person: person::ActiveModel = person.into();
        
        if let Some(name) = data.name {
            person.name = Set(name);
        }
        if let Some(birthdate) = data.birthdate {
            person.birthdate = Set(birthdate);
        }
        if let Some(dietary_goals) = data.dietary_goals {
            person.dietary_goals = Set(Some(dietary_goals));
        }
        if let Some(dislikes) = data.dislikes {
            person.dislikes = Set(serde_json::to_string(&dislikes).unwrap());
        }
        if let Some(favorites) = data.favorites {
            person.favorites = Set(serde_json::to_string(&favorites).unwrap());
        }
        if let Some(notes) = data.notes {
            person.notes = Set(Some(notes));
        }
        if let Some(is_active) = data.is_active {
            person.is_active = Set(is_active);
        }
        
        person.updated_at = Set(chrono::Utc::now());

        person.update(db).await
    }

    pub async fn delete(db: &DatabaseConnection, id: String) -> Result<(), DbErr> {
        Person::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}
```

Update `src-tauri/src/services/mod.rs`:

```rust
pub mod person_service;
```

**1.5: Create Tauri Commands**

Update `src-tauri/src/commands/person.rs` (add commands):

```rust
use tauri::State;
use crate::AppState;
use crate::services::person_service::PersonService;
use crate::entities::person;

#[tauri::command]
pub async fn get_all_people(state: State<'_, AppState>) -> Result<Vec<person::Model>, String> {
    PersonService::get_all(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_person(state: State<'_, AppState>, id: String) -> Result<Option<person::Model>, String> {
    PersonService::get_by_id(&state.db, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_person(
    state: State<'_, AppState>,
    data: CreatePersonDto,
) -> Result<person::Model, String> {
    PersonService::create(&state.db, data)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_person(
    state: State<'_, AppState>,
    id: String,
    data: UpdatePersonDto,
) -> Result<person::Model, String> {
    PersonService::update(&state.db, id, data)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_person(state: State<'_, AppState>, id: String) -> Result<(), String> {
    PersonService::delete(&state.db, id)
        .await
        .map_err(|e| e.to_string())
}
```

Update `src-tauri/src/commands/mod.rs`:

```rust
pub mod person;
```

**1.6: Set Up Database Connection**

File: `src-tauri/src/db.rs`

```rust
use sea_orm::{Database, DatabaseConnection, DbErr};
use std::path::PathBuf;

pub async fn init(app_handle: &tauri::AppHandle) -> Result<DatabaseConnection, DbErr> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");
    
    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");
    
    let db_path = app_data_dir.join("meal_planner.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    
    let db = Database::connect(&db_url).await?;
    
    // Run migrations
    migration::Migrator::up(&db, None).await?;
    
    Ok(db)
}
```

**1.7: Update main.rs**

File: `src-tauri/src/main.rs`

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod entities;
mod services;

use sea_orm::DatabaseConnection;

pub struct AppState {
    pub db: DatabaseConnection,
}

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let db = tauri::async_runtime::block_on(async {
                db::init(app.handle()).await.expect("Failed to initialize database")
            });

            app.manage(AppState { db });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::person::get_all_people,
            commands::person::get_person,
            commands::person::create_person,
            commands::person::update_person,
            commands::person::delete_person,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Verify Backend:** Run `npm run tauri dev` - should compile without errors

-----

### Frontend Tasks:

**1.8: Create TypeScript Types**

File: `src/types/person.ts`

```typescript
export interface Person {
  id: string
  name: string
  birthdate: string // ISO date string
  dietary_goals: string | null
  dislikes: string // JSON string
  favorites: string // JSON string
  notes: string | null
  is_active: boolean
  created_at: string
  updated_at: string
}

export interface CreatePersonDto {
  name: string
  birthdate: string
  dietary_goals?: string
  dislikes: string[]
  favorites: string[]
  notes?: string
}

export interface UpdatePersonDto {
  name?: string
  birthdate?: string
  dietary_goals?: string
  dislikes?: string[]
  favorites?: string[]
  notes?: string
  is_active?: boolean
}

// Helper to parse JSON fields
export function parsePerson(person: Person) {
  return {
    ...person,
    dislikes: JSON.parse(person.dislikes) as string[],
    favorites: JSON.parse(person.favorites) as string[],
  }
}
```

**1.9: Create API Hooks**

File: `src/hooks/usePeople.ts`

```typescript
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/tauri'
import type { Person, CreatePersonDto, UpdatePersonDto } from '../types/person'

export function usePeople() {
  return useQuery({
    queryKey: ['people'],
    queryFn: () => invoke<Person[]>('get_all_people'),
  })
}

export function usePerson(id: string) {
  return useQuery({
    queryKey: ['people', id],
    queryFn: () => invoke<Person | null>('get_person', { id }),
    enabled: !!id,
  })
}

export function useCreatePerson() {
  const queryClient = useQueryClient()
  
  return useMutation({
    mutationFn: (data: CreatePersonDto) =>
      invoke<Person>('create_person', { data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['people'] })
    },
  })
}

export function useUpdatePerson() {
  const queryClient = useQueryClient()
  
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdatePersonDto }) =>
      invoke<Person>('update_person', { id, data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['people'] })
    },
  })
}

export function useDeletePerson() {
  const queryClient = useQueryClient()
  
  return useMutation({
    mutationFn: (id: string) => invoke('delete_person', { id }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['people'] })
    },
  })
}
```

**1.10: Create Family Manager Component**

File: `src/components/FamilyManager.tsx`

```typescript
import { useState } from 'react'
import { usePeople, useCreatePerson, useUpdatePerson, useDeletePerson } from '../hooks/usePeople'
import { parsePerson } from '../types/person'
import type { CreatePersonDto } from '../types/person'

export function FamilyManager() {
  const { data: people, isLoading } = usePeople()
  const createMutation = useCreatePerson()
  const updateMutation = useUpdatePerson()
  const deleteMutation = useDeletePerson()

  const [isAdding, setIsAdding] = useState(false)
  const [newPerson, setNewPerson] = useState<CreatePersonDto>({
    name: '',
    birthdate: '',
    dislikes: [],
    favorites: [],
  })

  const handleCreate = async () => {
    await createMutation.mutateAsync(newPerson)
    setIsAdding(false)
    setNewPerson({ name: '', birthdate: '', dislikes: [], favorites: [] })
  }

  if (isLoading) return <div>Loading...</div>

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-4">Family Members</h1>
      
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {people?.map((person) => {
          const parsed = parsePerson(person)
          return (
            <div key={person.id} className="border p-4 rounded">
              <h3 className="font-bold">{person.name}</h3>
              <p className="text-sm text-gray-600">Born: {person.birthdate}</p>
              {person.dietary_goals && (
                <p className="text-sm">Goals: {person.dietary_goals}</p>
              )}
              {parsed.dislikes.length > 0 && (
                <p className="text-sm">Dislikes: {parsed.dislikes.join(', ')}</p>
              )}
              {parsed.favorites.length > 0 && (
                <p className="text-sm">Favorites: {parsed.favorites.join(', ')}</p>
              )}
              <button
                onClick={() => deleteMutation.mutate(person.id)}
                className="mt-2 text-red-600 text-sm"
              >
                Delete
              </button>
            </div>
          )
        })}
      </div>

      {isAdding ? (
        <div className="mt-4 border p-4 rounded">
          <h3 className="font-bold mb-2">Add Person</h3>
          <input
            type="text"
            placeholder="Name"
            value={newPerson.name}
            onChange={(e) => setNewPerson({ ...newPerson, name: e.target.value })}
            className="border p-2 rounded w-full mb-2"
          />
          <input
            type="date"
            value={newPerson.birthdate}
            onChange={(e) => setNewPerson({ ...newPerson, birthdate: e.target.value })}
            className="border p-2 rounded w-full mb-2"
          />
          <div className="flex gap-2">
            <button onClick={handleCreate} className="bg-blue-600 text-white px-4 py-2 rounded">
              Save
            </button>
            <button onClick={() => setIsAdding(false)} className="border px-4 py-2 rounded">
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <button
          onClick={() => setIsAdding(true)}
          className="mt-4 bg-blue-600 text-white px-4 py-2 rounded"
        >
          + Add Person
        </button>
      )}
    </div>
  )
}
```

**1.11: Update App.tsx**

```typescript
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { FamilyManager } from './components/FamilyManager'

const queryClient = new QueryClient()

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <div className="min-h-screen bg-gray-50">
        <FamilyManager />
      </div>
    </QueryClientProvider>
  )
}

export default App
```

**Verify Phase 1:**

- Run `npm run tauri dev`
- Should see family manager UI
- Create, view, delete people
- Check SQLite database file was created in app data directory

**Deliverable:** Working Person CRUD with UI

-----

## Phase 2: Recipe Entity (CRUD)

**Goal:** Complete Recipe management - backend + frontend

### Backend Tasks:

**2.1: Create Recipe Migration**

File: `src-tauri/migration/src/m20260118_000002_create_recipes.rs`

Create migration with all Recipe fields including:

- id, name, description, source, parent_recipe_id
- prep_time, cook_time, total_time (JSON text)
- servings, portion_size (JSON text)
- instructions (text)
- ingredients (JSON text array)
- nutrition_per_serving (JSON text)
- tags (JSON text array)
- notes, icon
- is_favorite, times_made, last_made
- created_at, updated_at

**2.2: Create Recipe Entity**

File: `src-tauri/src/entities/recipe.rs`

Similar to Person entity, with JSON fields for all complex structures.

**2.3: Create Recipe DTOs**

File: `src-tauri/src/commands/recipe.rs`

Include structs for:

- TimeValue { value: i32, unit: String }
- PortionSize { value: f64, unit: String }
- IngredientAmount (enum: Single{value: f64} | Range{min: f64, max: f64})
- Ingredient { name, amount, unit, notes }
- Nutrition { calories, protein_grams, carbs_grams, fat_grams, notes }
- CreateRecipeDto
- UpdateRecipeDto

**2.4: Create Recipe Service**

File: `src-tauri/src/services/recipe_service.rs`

CRUD operations plus:

- Search/filter by name, tags
- Mark as favorite
- Increment times_made

**2.5: Create Tauri Commands**

Add to main.rs:

- get_all_recipes
- get_recipe
- create_recipe
- update_recipe
- delete_recipe
- search_recipes

**2.6: Add Markdown Import Logic**

File: `src-tauri/src/services/recipe_parser.rs`

Function to parse markdown format into Recipe struct:

```
# Recipe Name
Description
Prep time: X min
Servings: Y

## Ingredients
- amount unit ingredient name

## Instructions
Step by step...
```

### Frontend Tasks:

**2.7: Create TypeScript Types**

File: `src/types/recipe.ts`

Match all Rust structures (TimeValue, Ingredient, etc.)

**2.8: Create API Hooks**

File: `src/hooks/useRecipes.ts`

Similar to usePeople hooks

**2.9: Create Recipe Manager Component**

File: `src/components/RecipeManager.tsx`

Features:

- Recipe list with search
- Add recipe form (manual entry)
- Import from markdown modal
- Edit recipe
- Delete recipe
- View recipe detail

**2.10: Add Navigation**

Update App.tsx to have tabs/nav between Family and Recipes

**Verify Phase 2:**

- Create recipes manually
- Import recipe from markdown
- Edit and delete recipes
- Search/filter recipes
- Mark favorites

**Deliverable:** Working Recipe CRUD with UI

-----

## Phase 3: Meal Entity & Planning

**Goal:** Meal planning with recipe assignment per person

### Backend Tasks:

**3.1: Create Meal Migration**

File: `src-tauri/migration/src/m20260118_000003_create_meals.rs`

Create meals table with:

- id, date, meal_type, order_index
- servings (JSON text - array of PersonServing objects)
- created_at, updated_at

**3.2: Create Meal Entity**

File: `src-tauri/src/entities/meal.rs`

**3.3: Create Meal DTOs**

File: `src-tauri/src/commands/meal.rs`

Structs for:

- PersonServing (enum: Recipe{recipe_id, servings_count, notes} | Adhoc{adhoc_items: Vec<Ingredient>, notes})
- CreateMealDto
- UpdateMealDto

**3.4: Create Meal Service**

File: `src-tauri/src/services/meal_service.rs`

Features:

- CRUD operations
- Get meals for date range
- When creating/updating meal with recipes, increment recipe.times_made

**3.5: Create Tauri Commands**

Add to main.rs:

- get_meals_for_date_range
- get_meal
- create_meal
- update_meal
- delete_meal

### Frontend Tasks:

**3.6: Create TypeScript Types**

File: `src/types/meal.ts`

**3.7: Create API Hooks**

File: `src/hooks/useMeals.ts`

**3.8: Create Calendar Component**

File: `src/components/Calendar.tsx`

Features:

- Weekly view (7 day grid)
- Each day shows meals (Breakfast, Lunch, Dinner + custom)
- Click meal to edit
- Navigate prev/next week
- Highlight today

**3.9: Create Meal Editor Modal**

File: `src/components/MealEditor.tsx`

Features:

- Shows meal type and date
- List all family members
- For each person:
  - Select recipe from dropdown (with servings_count input)
  - OR add adhoc items
- Save/cancel buttons

**3.10: Add Navigation**

Update App.tsx to add Calendar tab

**Verify Phase 3:**

- View weekly calendar
- Create meals for different days
- Assign different recipes to different people
- Add adhoc food items for kids
- Edit and delete meals
- Verify recipe.times_made increments when recipe is used
- Add custom meal types (Snack, Dessert)

**Deliverable:** Working meal planning system

-----

## Phase 4: Shopping List

**Goal:** Compute aggregated shopping list from date range

### Backend Tasks:

**4.1: Create Shopping Service**

File: `src-tauri/src/services/shopping_service.rs`

Main function: `get_shopping_list(db, start_date, end_date)`

Logic:

1. Query all meals in date range
1. For each PersonServing:
- If recipe: get recipe, scale ingredients by servings_count
- If adhoc: use adhoc_items directly
1. Group ingredients by name (case-insensitive)
1. For each ingredient group:
- Convert units to common base (use unit_converter)
- Sum amounts (handle single values and ranges)
- Return total with most common unit

**4.2: Add Unit Conversion Module**

File: `src-tauri/src/services/unit_converter.rs`

Add `uom` crate to Cargo.toml:

```toml
uom = "0.36"
```

Create conversion functions:

- Weight conversions (g, kg, oz, lb)
- Volume conversions (ml, L, cup, tbsp, tsp, fl oz)
- No conversion for discrete units (whole, piece, etc.)

**4.3: Create Return Types**

File: `src-tauri/src/commands/shopping.rs`

```rust
#[derive(Serialize)]
pub struct AggregatedIngredient {
    pub ingredient_name: String,
    pub total_amount: Option<IngredientAmount>, // Some if all items convertible
    pub total_unit: Option<String>,
    pub items: Vec<IngredientSource>,
}

#[derive(Serialize)]
pub struct IngredientSource {
    pub amount: IngredientAmount,
    pub unit: String,
    pub source_type: String, // "recipe" or "adhoc"
    pub source_id: Option<String>, // recipe_id if source_type is recipe
    pub meal_id: String,
}
```

**4.4: Create Tauri Command**

Add to main.rs:

- get_shopping_list(start_date: String, end_date: String) -> Vec<AggregatedIngredient>

### Frontend Tasks:

**4.5: Create TypeScript Types**

File: `src/types/shopping.ts`

Match backend types

**4.6: Create API Hook**

File: `src/hooks/useShopping.ts`

```typescript
export function useShoppingList(startDate: string, endDate: string) {
  return useQuery({
    queryKey: ['shopping', startDate, endDate],
    queryFn: () => invoke<AggregatedIngredient[]>('get_shopping_list', { 
      startDate, 
      endDate 
    }),
    enabled: !!startDate && !!endDate,
  })
}
```

**4.7: Create Shopping List Component**

File: `src/components/ShoppingList.tsx`

Features:

- Date range picker (start/end date inputs)
- Display aggregated ingredients
- For each ingredient:
  - Show total amount and unit
  - Expandable to see breakdown by source
  - Show which meals need it
- Group by ingredient name
- Show “no meals planned” if empty

**4.8: Add Navigation**

Update App.tsx to add Shopping tab

**Verify Phase 4:**

- Select date range with planned meals
- See aggregated ingredients
- Verify unit conversions work (e.g., 2 cups flour + 250g flour = X grams total)
- Verify ranges are summed correctly
- Verify adhoc items are included
- Check breakdown shows all sources

**Deliverable:** Working shopping list with unit conversion

-----

## Phase 5: Polish & Initial Data

**Goal:** Make it ready for daily use

### Tasks:

**5.1: Seed Initial Family Data**

Create a seed function that adds your 4 family members on first run:

- Steve (born 1978)
- Wife (born 1980)
- Daughter 1 (born 2014)
- Daughter 2 (born 2019)

**5.2: Improve UI/UX**

- Add loading spinners for all async operations
- Add error messages when operations fail
- Add confirmation dialogs for delete operations
- Improve form validation
- Better empty states
- Add keyboard shortcuts (ESC to close modals, etc.)

**5.3: Better Styling**

- Consistent color scheme with Tailwind
- Use shadcn/ui components (optional but nice)
- Responsive design (works on different window sizes)
- Icons from lucide-react
- Better spacing and typography

**5.4: Recipe Icons**

Add emoji picker or predefined icon list for recipes

**5.5: Date Helpers**

Create utility functions for:

- Formatting dates consistently
- Calculating week ranges
- Getting current week

**5.6: Default Meals**

When viewing a new day, auto-create Breakfast/Lunch/Dinner placeholders

**5.7: README & Documentation**

Create comprehensive README.md with:

- What is this app
- How to install dependencies
- How to run in dev mode
- How to build for production
- Basic usage guide
- Screenshots

**5.8: Error Handling**

Improve error handling throughout:

- Better error messages from backend
- User-friendly error display in UI
- Log errors for debugging

**Verify Phase 5:**

- App looks polished
- No crashes or confusing states
- Easy to understand how to use
- Documentation is clear

**Deliverable:** Production-ready MVP

-----

## Task Breakdown for Claude Code

Give these tasks to Claude Code in order:

1. **“Set up Tauri + React + SeaORM project with proper structure (Phase 0)”**
- Initialize project
- Add all dependencies
- Create directory structure
- Verify build works
1. **“Implement Person entity backend (Phase 1, tasks 1.1-1.7)”**
- Create migration
- Create entity
- Create service
- Create commands
- Wire up main.rs
1. **“Implement Person entity frontend (Phase 1, tasks 1.8-1.11)”**
- Create types
- Create hooks
- Create UI component
- Test CRUD operations
1. **“Implement Recipe entity backend (Phase 2, tasks 2.1-2.6)”**
- Similar to Person but with more complex structures
- Include markdown import
1. **“Implement Recipe entity frontend (Phase 2, tasks 2.7-2.10)”**
- Recipe list, add, edit, delete
- Markdown import UI
1. **“Implement Meal entity backend (Phase 3, tasks 3.1-3.5)”**
- Most complex entity
- Handle PersonServing with recipe/adhoc variants
1. **“Implement Meal entity frontend (Phase 3, tasks 3.6-3.10)”**
- Calendar view
- Meal editor modal
- Recipe assignment per person
1. **“Implement Shopping List (Phase 4)”**
- Backend aggregation logic
- Unit conversion
- Frontend display
1. **“Polish and finalize (Phase 5)”**
- Seed data
- UI improvements
- Documentation

-----

## Testing Checkpoints

After each phase, manually test:

**Phase 1:**

- [ ] Can create a person
- [ ] Can view all people
- [ ] Can edit a person
- [ ] Can delete a person
- [ ] Data persists after app restart

**Phase 2:**

- [ ] Can create recipe manually
- [ ] Can import recipe from markdown
- [ ] Can edit recipe
- [ ] Can delete recipe
- [ ] Recipes show in searchable list

**Phase 3:**

- [ ] Can view weekly calendar
- [ ] Can create meal on specific day
- [ ] Can assign recipe to person with serving count
- [ ] Can add adhoc items for person
- [ ] Different people can have different food
- [ ] Recipe times_made increments

**Phase 4:**

- [ ] Can select date range
- [ ] Shopping list shows all ingredients
- [ ] Ingredients are grouped correctly
- [ ] Units are converted properly
- [ ] Totals are calculated correctly

**Phase 5:**

- [ ] App looks polished
- [ ] No obvious bugs
- [ ] Documentation is complete

-----

## Notes

- Each phase builds on the previous one
- Test thoroughly before moving to next phase
- Don’t hesitate to refactor if something feels wrong
- UI can start simple and get polished in Phase 5
- Focus on functionality first, polish later