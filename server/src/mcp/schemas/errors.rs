//! Error types that flow back through the MCP boundary. Tool handlers wrap
//! these via `tool_user_error(e.to_string())` so the Display string surfaces
//! as a tool-level `CallToolResult { is_error: true, … }`. JSON-RPC protocol
//! errors get rendered as a generic "Tool execution failed" by most MCP
//! clients, with the message dropped — tool-level errors carry the
//! actionable text through to the LLM.

use crate::services::recipe_variant::VariantError;

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
    /// Carries the rating exactly as the caller sent it.
    //
    // The range check runs on the rounded value, so reporting that instead
    // would reject 5.6 with "(got 6)" — a number the caller never sent.
    RatingOutOfRange(f64),
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
    /// Specialized variant for the printable tool's tighter cap. The
    /// generic [`DateRangeTooWide`] message mentions "multi-megabyte
    /// responses" — accurate for the 366-day cap but misleading here,
    /// where the real reason is a single-sheet print fit. `days_inclusive`
    /// and `max_days_inclusive` are inclusive day counts (end − start + 1)
    /// so the error message matches the way the docs describe the cap
    /// ("14 inclusive days") rather than the span ("end − start" = 13).
    PrintableSpanTooWide {
        days_inclusive: i64,
        max_days_inclusive: i64,
    },
    /// A field that's documented as non-empty (overlay `prep_notes` entry,
    /// `use_up_notes` entry, `DontForgetItem.prefix`/`body`) was passed as
    /// an empty / whitespace-only string. Surfaced as a validation error so
    /// the LLM gets clear feedback rather than silently dropping the field.
    EmptyOverlayField {
        field: &'static str,
    },
    /// An overlay collection has more items than the printable can fit on a
    /// single US Letter sheet. Each entry adds a fixed vertical chunk; this
    /// is the only validation strong enough to prevent runaway overflow.
    OverlayListTooLong {
        field: &'static str,
        count: usize,
        max_count: usize,
    },
    /// An `adapt_recipe` call carries no ingredient or instruction change.
    NoAdaptationChanges,
    /// An `adapt_recipe` call sends both `instructions` and
    /// `instruction_edits`.
    ConflictingInstructionChanges,
    /// An `adapt_recipe` ingredient change, numbered from 1, names no
    /// ingredient to replace or remove.
    EmptyIngredientChangeName(usize),
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
            Self::RatingOutOfRange(n) => write!(
                f,
                "rating must be a whole number from 1 to 5 (got {n}). Fractional values round to the nearest star. Call unrate_recipe to remove a rating entirely — 0 is not a rating."
            ),
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
            Self::PrintableSpanTooWide {
                days_inclusive,
                max_days_inclusive,
            } => write!(
                f,
                "printable covers {days_inclusive} days, exceeding the {max_days_inclusive}-day cap. Narrow start_date / end_date — fridge-card printables are sized for a single US Letter sheet, and wider windows overflow onto a second page."
            ),
            Self::EmptyOverlayField { field } => write!(
                f,
                "{field} must not be empty or whitespace-only. Either supply a value or omit the field."
            ),
            Self::OverlayListTooLong {
                field,
                count,
                max_count,
            } => write!(
                f,
                "{field} has {count} items, exceeding the {max_count}-item cap. Trim the list — each item adds a fixed vertical chunk to the printable, and wider lists overflow onto a second page."
            ),
            Self::NoAdaptationChanges => write!(
                f,
                "adapt_recipe needs at least one ingredient_changes op or an instruction change (instruction_edits or instructions). To change only the name, description, tags, or notes, call update_recipe on the parent, or create_recipe with parent_recipe_slug to save a linked copy."
            ),
            Self::EmptyIngredientChangeName(number) => write!(
                f,
                "ingredient change #{number}: name must not be empty or whitespace-only. Give the whole name of the ingredient to replace or remove, as get_recipe shows it."
            ),
            Self::ConflictingInstructionChanges => write!(
                f,
                "send instruction_edits or instructions, not both. Use instruction_edits for targeted find/replace changes, or instructions to replace the whole text."
            ),
        }
    }
}

/// Render a [`VariantError`] for the LLM, adding the MCP tool call that
/// recovers from it. `parent_slug` is the canonical slug of the recipe being
/// adapted.
//
// `VariantError` lives in the service layer and names no tools, so the MCP
// recovery advice is attached here. A malformed parent is a server fault that
// the handler reports as an internal error; it gets no hint. A change after
// the first one runs against what the earlier changes left, which `get_recipe`
// on the parent does not show, so its hint says so.
pub fn variant_error_message(err: &VariantError, parent_slug: &str) -> String {
    let (number, hint) = match err {
        VariantError::AmbiguousIngredient {
            number,
            separable_by_prep: false,
            ..
        } => (
            *number,
            format!("No adapt_recipe change can target just one of them. Call create_recipe with parent_recipe_slug '{parent_slug}' and the full ingredient list instead, or leave this change out and fix the saved variant's ingredients with update_recipe."),
        ),
        VariantError::UnmatchedIngredient { number, .. }
        | VariantError::AmbiguousIngredient { number, .. } => (
            *number,
            format!("Call get_recipe on '{parent_slug}' for its current ingredient names."),
        ),
        VariantError::EmptyEditFind { number }
        | VariantError::EditNotFound { number, .. }
        | VariantError::EditAmbiguous { number, .. } => (
            *number,
            format!("Call get_recipe on '{parent_slug}' for its current instruction text."),
        ),
        VariantError::MalformedParent(_) => return err.to_string(),
    };
    if number > 1 {
        format!("{err} {hint} The earlier changes in this call apply first, so #{number} sees what they left rather than the parent as stored.")
    } else {
        format!("{err} {hint}")
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
