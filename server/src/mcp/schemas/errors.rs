//! Error types that flow back through the MCP boundary. Tool handlers wrap
//! these into `McpError::invalid_params` (or similar) with the Display string
//! as the user-facing message.

/// Canonical values for `Meal.meal_type` as stored in the DB — Title Case
/// to match the convention used by the rest of the app (the web UI does
/// strict `meal_type === 'Dinner'` equality in `MealPlanner.tsx`). MCP
/// input is matched case-insensitively against this list and then
/// normalized to the canonical form before storage — see
/// [`canonical_meal_type`](super::meals::canonical_meal_type).
pub const VALID_MEAL_TYPES: &[&str] = &["Breakfast", "Lunch", "Dinner", "Snack"];

/// Error returned when a `create_meal` input references a person name or
/// recipe slug that doesn't exist. The tool handler converts this into an
/// `invalid_params` MCP error so the LLM retries with a corrected value.
#[derive(Debug)]
pub enum ResolveError {
    UnknownPerson(String),
    UnknownRecipe(String),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPerson(name) => write!(
                f,
                "no active family member named '{name}'. Call list_people to see valid names."
            ),
            Self::UnknownRecipe(slug) => write!(
                f,
                "no recipe with slug '{slug}'. Call search_recipes or list_recipes to see valid slugs."
            ),
        }
    }
}

/// Error returned when a write-tool input fails one of the semantic checks
/// we apply in addition to JSON-schema validation.
#[derive(Debug)]
pub enum InputError {
    NonPositiveServings(i32),
    NonPositiveServingsCount(f64),
    UnknownMealType(String),
    EmptyName(&'static str),
    InvalidDate { field: &'static str, value: String },
}

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonPositiveServings(n) => write!(
                f,
                "servings must be >= 1 (got {n}). Recipes need at least one serving so shopping-list scaling works."
            ),
            Self::NonPositiveServingsCount(n) => write!(
                f,
                "servings_count must be > 0 (got {n}). Use 0.5 for a half portion; negative or zero values would corrupt the shopping list."
            ),
            Self::UnknownMealType(mt) => write!(
                f,
                "meal_type must be one of Breakfast, Lunch, Dinner, or Snack (case-insensitive; got '{mt}')."
            ),
            Self::EmptyName(field) => write!(f, "{field} must not be empty or whitespace-only."),
            Self::InvalidDate { field, value } => write!(
                f,
                "{field} must be in YYYY-MM-DD format (got '{value}')."
            ),
        }
    }
}

/// Error returned when a `create_meal` input fails validation OR references
/// a person name or recipe slug that doesn't exist.
#[derive(Debug)]
pub enum CreateMealError {
    Input(InputError),
    Resolve(ResolveError),
}

impl std::fmt::Display for CreateMealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(e) => write!(f, "{e}"),
            Self::Resolve(e) => write!(f, "{e}"),
        }
    }
}

impl From<InputError> for CreateMealError {
    fn from(e: InputError) -> Self {
        Self::Input(e)
    }
}

impl From<ResolveError> for CreateMealError {
    fn from(e: ResolveError) -> Self {
        Self::Resolve(e)
    }
}
