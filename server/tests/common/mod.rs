// Helpers shared by the integration tests. Each test binary compiles its own
// copy and uses only some of it.
#![allow(dead_code)]

use fewd_lib::dto::{CreateDrinkRecipeDto, CreateRecipeDto, IngredientAmountDto, IngredientDto};

/// Holds a file-backed database path unique to one test in this process and
/// removes the file and its WAL sidecars on creation and on drop. Use a file
/// rather than `:memory:` when several pooled connections must share one
/// database.
pub struct TempDbPath(pub String);

impl TempDbPath {
    /// Builds the path for `name`, which must be unique across the test
    /// binary because its tests share one process id.
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("fewd_{name}_{}.db", std::process::id()))
            .to_string_lossy()
            .into_owned();
        let me = Self(path);
        // A crashed run with a recycled process id can leave a stale database.
        me.remove_files();
        me
    }

    fn remove_files(&self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.0, suffix));
        }
    }
}

impl Drop for TempDbPath {
    fn drop(&mut self) {
        self.remove_files();
    }
}

/// Returns one cup of the named ingredient.
pub fn ingredient(name: &str) -> IngredientDto {
    IngredientDto {
        name: name.to_string(),
        prep: None,
        amount: IngredientAmountDto::Single { value: 1.0 },
        unit: "cup".to_string(),
        notes: None,
        or_alternative: None,
    }
}

/// Returns a recipe with one ingredient that create accepts.
pub fn recipe_dto(name: &str) -> CreateRecipeDto {
    CreateRecipeDto {
        name: name.to_string(),
        description: None,
        source: "manual".to_string(),
        source_url: None,
        parent_recipe_id: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: 4,
        portion_size: None,
        instructions: "Mix and cook".to_string(),
        ingredients: vec![ingredient("flour")],
        nutrition_per_serving: None,
        tags: vec![],
        notes: None,
        icon: None,
    }
}

/// Returns a drink with one ingredient that create accepts.
pub fn drink_dto(name: &str) -> CreateDrinkRecipeDto {
    CreateDrinkRecipeDto {
        name: name.to_string(),
        description: None,
        source: "manual".to_string(),
        source_url: None,
        servings: 1,
        instructions: "Shake".to_string(),
        ingredients: vec![ingredient("rum")],
        technique: None,
        glassware: None,
        garnish: None,
        tags: vec![],
        notes: None,
        icon: None,
        is_non_alcoholic: None,
    }
}
