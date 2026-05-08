//! Error types that flow back through the MCP boundary. Tool handlers wrap
//! these via `tool_user_error(e.to_string())` so the Display string surfaces
//! as a tool-level `CallToolResult { is_error: true, … }`. JSON-RPC protocol
//! errors get rendered as a generic "Tool execution failed" by most MCP
//! clients, with the message dropped — tool-level errors carry the
//! actionable text through to the LLM.

/// Canonical values for `Meal.meal_type` as stored in the DB — Title Case
/// to match the convention used by the rest of the app (the web UI does
/// strict `meal_type === 'Dinner'` equality in `MealPlanner.tsx`). MCP
/// input is matched case-insensitively against this list and then
/// normalized to the canonical form before storage — see
/// [`canonical_meal_type`](super::meals::canonical_meal_type).
pub const VALID_MEAL_TYPES: &[&str] = &["Breakfast", "Lunch", "Dinner", "Snack"];

/// Error returned when a `create_meal` input references a person name or
/// recipe slug that doesn't exist. The tool handler routes this through
/// `tool_user_error` so the Display string reaches the LLM as actionable
/// retry guidance pointing at the relevant discovery tool.
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
                "no recipe with slug '{slug}'. Call list_curated_recipes for the shortlist or search_recipes with a filter to see valid slugs."
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
    InvalidDate {
        field: &'static str,
        value: String,
    },
    ReversedDateRange {
        start_date: String,
        end_date: String,
    },
    DateRangeTooWide {
        days: i64,
        max_days: i64,
    },
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
            Self::ReversedDateRange {
                start_date,
                end_date,
            } => write!(
                f,
                "end_date ('{end_date}') must be on or after start_date ('{start_date}'). Swap them or pick a forward range."
            ),
            Self::DateRangeTooWide { days, max_days } => write!(
                f,
                "date range spans {days} days, exceeding the {max_days}-day per-call cap. Narrow start_date / end_date and call again — wider sweeps would fan out into a multi-megabyte response."
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
