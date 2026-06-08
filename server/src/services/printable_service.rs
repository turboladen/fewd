//! Render an HTML fridge-card printable from scheduled meals + LLM-supplied
//! overlay annotations. The canonical layout lives in
//! [`template.html`](./printable/template.html); this module builds the
//! per-day rows and overlay blocks, then substitutes them into the template.
//!
//! ## Single-page constraint
//!
//! The template is sized to fit one US Letter portrait sheet at 100% browser
//! print scale. The renderer cooperates by:
//!
//! - Capping the date range upstream at 14 inclusive days
//!   ([`MAX_PRINTABLE_DAYS`](crate::mcp::schemas::printable::MAX_PRINTABLE_DAYS)).
//! - Letting the template clip overlong blurbs via `-webkit-line-clamp`
//!   rather than allowing content to push the page.
//! - Pairing `page-break-inside: avoid` on row + reminder elements with a
//!   compact type scale so the natural break points are well-defined.
//!
//! ## Determinism contract
//!
//! Given the same `(meals, recipe_models, person_names, validated, input)`,
//! output is byte identical. The renderer never reads the system clock; the
//! footer's subtitle comes from `input.foot_note` when supplied, else from a
//! deterministic format derived from the validated range and slot list.
//! This is what makes the snapshot tests below stable.
//!
//! ## Soft-failure observability
//!
//! Several conditions degrade rendering rather than failing it outright:
//!
//! - A meal whose `servings` JSON fails to parse → that single row renders
//!   as a `(corrupt meal data)` placeholder, an `error!` is logged with
//!   `meal_id` + parse error, and the rest of the week still renders. The
//!   alternative (failing the whole render) would deny the family their
//!   printable over a single bad row.
//! - A meal referencing a soft-deleted `person_id` or a deleted `recipe_id`
//!   → placeholder text (`(inactive person, id=…)` / `(deleted recipe,
//!   id=…)`) renders in-place and a `warn!` is logged with the meal id and
//!   reference id. The placeholders match the wording used by
//!   [`mcp::schemas::meals::meal_to_brief`] so log searches catch both
//!   surfaces with one query.
//! - A recipe with corrupt `total_time` JSON → the time-tag is omitted and a
//!   `warn!` is logged with the recipe id and parse error.

use std::collections::HashMap;
use std::fmt::Write as _;

use chrono::{Datelike, NaiveDate};

use crate::dto::{MealType, PersonServingDto, TimeValueDto};
use crate::entities::{meal, recipe};
use crate::mcp::schemas::printable::{DayOverlay, DontForgetItem, PrintableInput, ValidatedRange};

/// Person-id → display name. The handler builds this from
/// [`PersonService::get_all`](crate::services::person_service::PersonService::get_all)
/// once per call, so the renderer never touches the DB. Passing a plain map
/// (rather than the MCP-internal `MealLookups`) keeps the service layer
/// agnostic to the MCP module's private types.
pub type PersonNameMap = std::collections::HashMap<String, String>;

const TEMPLATE: &str = include_str!("printable/template.html");

/// Render the canonical fridge-card printable.
///
/// `recipe_models` maps `recipe_id → recipe::Model`. Callers build it by
/// calling [`RecipeService::get_all`](crate::services::recipe_service::RecipeService::get_all)
/// once and collecting; meals reference recipes by id, so the map must cover
/// every recipe scheduled in the window.
pub fn render(
    meals: &[meal::Model],
    recipe_models: &HashMap<String, recipe::Model>,
    person_names: &PersonNameMap,
    validated: &ValidatedRange,
    input: &PrintableInput,
) -> String {
    let included_slots: std::collections::HashSet<MealType> =
        validated.include.iter().copied().collect();

    let visible_meals: Vec<&meal::Model> = meals
        .iter()
        .filter(|m| included_slots.contains(&m.meal_type))
        .collect();

    let overlay_by_date: HashMap<NaiveDate, &DayOverlay> = input
        .day_overlays
        .iter()
        .filter(|ov| ov.date >= validated.start && ov.date <= validated.end)
        .map(|ov| (ov.date, ov))
        .collect();

    let nights_or_empty = if visible_meals.is_empty() {
        render_empty_state(validated)
    } else {
        render_nights(
            &visible_meals,
            recipe_models,
            person_names,
            &overlay_by_date,
            input,
        )
    };

    let header_right = render_header_right(input);
    let reminders = render_reminders(&input.dont_forget);
    let foot_note = input
        .foot_note
        .clone()
        .unwrap_or_else(|| default_foot_note(validated));
    let week_label = format_week_label(validated.start, validated.end);
    let slot_label = slot_label(&validated.include);
    let head_title = format!("{slot_label} Plan — {week_label}");
    let (title_top, title_bottom) = title_lines(&validated.include);

    // Single-pass substitution: walk the *template* once and emit the
    // mapped value for each `{{NAME}}` token. Naive `template.replace(a, x)
    // .replace(b, y)` chains let a `b`-shaped substring of `x` get rewritten
    // in the next pass — so a user-supplied `week_theme = "{{NIGHTS_OR_EMPTY}}"`
    // would corrupt the badge with the day-rows block. Walking the template
    // once avoids that entire class of collision because inserted values
    // are never re-scanned.
    let head_title_esc = esc_text(&head_title);
    let title_top_esc = esc_text(title_top);
    let title_bottom_esc = esc_text(&title_bottom);
    let week_label_esc = esc_text(&week_label);
    let foot_note_esc = esc_text(&foot_note);
    substitute(
        TEMPLATE,
        &[
            ("HEAD_TITLE", &head_title_esc),
            ("TITLE_TOP", &title_top_esc),
            ("TITLE_BOTTOM", &title_bottom_esc),
            ("WEEK_LABEL", &week_label_esc),
            ("HEADER_RIGHT", &header_right),
            ("NIGHTS_OR_EMPTY", &nights_or_empty),
            ("REMINDERS_BLOCK", &reminders),
            ("FOOT_NOTE", &foot_note_esc),
        ],
    )
}

/// Single-pass `{{NAME}}` substitution. Walks `template` once: literal text
/// is appended to the output, each `{{NAME}}` is replaced by the matching
/// entry's value from `replacements`, and the inserted value is **never
/// re-scanned**. That last property is what makes user-supplied content
/// like `week_theme = "{{NIGHTS_OR_EMPTY}}"` safe — it survives as literal
/// text in the output rather than getting picked up by a later replacement.
///
/// Unknown placeholders (in the template but not in `replacements`) are
/// emitted verbatim with the surrounding `{{ }}` and an `error!` log; that
/// way a typo in the template shows up as a visible debug marker rather
/// than silently dropping content. An unclosed `{{` (no matching `}}`)
/// emits the remaining template verbatim for the same reason.
fn substitute(template: &str, replacements: &[(&'static str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 8 * 1024);
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + 2..];
        match after_open.find("}}") {
            Some(close) => {
                let name = &after_open[..close];
                match replacements.iter().find(|(k, _)| *k == name) {
                    Some((_, value)) => out.push_str(value),
                    None => {
                        tracing::error!(
                            placeholder = name,
                            "printable: template references an unknown placeholder; \
                             leaving literal in output as debug marker"
                        );
                        out.push_str("{{");
                        out.push_str(name);
                        out.push_str("}}");
                    }
                }
                rest = &after_open[close + 2..];
            }
            None => {
                // Unclosed `{{`. Emit the rest of the template verbatim
                // (including the dangling `{{`) so the bug is visible in
                // the output rather than silently truncated.
                out.push_str(&rest[open..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

fn render_empty_state(validated: &ValidatedRange) -> String {
    format!(
        "<div class=\"empty-state\">No meals scheduled for {} — call \
         <code>create_meal</code> to populate the week first.</div>",
        esc_text(&format_week_label(validated.start, validated.end))
    )
}

fn render_header_right(input: &PrintableInput) -> String {
    if input.week_theme.is_none() && input.use_up_notes.is_empty() {
        return String::new();
    }
    let mut html = String::from("<div class=\"header-right\">");
    if let Some(theme) = &input.week_theme {
        let _ = write!(html, "<div class=\"badge\">{}</div>", esc_text(theme));
    }
    if !input.use_up_notes.is_empty() {
        html.push_str("<div class=\"use-up\">");
        for note in &input.use_up_notes {
            let _ = write!(html, "<div>{}</div>", esc_text(note));
        }
        html.push_str("</div>");
    }
    html.push_str("</div>");
    html
}

fn render_nights(
    meals: &[&meal::Model],
    recipe_models: &HashMap<String, recipe::Model>,
    person_names: &PersonNameMap,
    overlay_by_date: &HashMap<NaiveDate, &DayOverlay>,
    input: &PrintableInput,
) -> String {
    let mut html = String::from("<div class=\"nights\">");
    for m in meals {
        let overlay = overlay_by_date.get(&m.date).copied();
        html.push_str(&render_night(
            m,
            recipe_models,
            person_names,
            overlay,
            input,
        ));
    }
    html.push_str("</div>");
    html
}

fn render_night(
    m: &meal::Model,
    recipe_models: &HashMap<String, recipe::Model>,
    person_names: &PersonNameMap,
    overlay: Option<&DayOverlay>,
    input: &PrintableInput,
) -> String {
    let day_name = m.date.format("%a").to_string();
    let day_date = m.date.format("%b %-d").to_string();
    let day_tag_html = overlay
        .and_then(|o| o.tag.as_deref())
        .map(|t| format!("<div class=\"day-tag\">{}</div>", esc_text(t)))
        .unwrap_or_default();

    // Per-row fallback so a single corrupt servings JSON degrades that row
    // rather than killing the whole printable. See module docs (Soft-failure
    // observability).
    let (title_block, blurb, tags) =
        match serde_json::from_str::<Vec<PersonServingDto>>(&m.servings) {
            Ok(servings) => (
                render_meal_title(&m.id, &servings, recipe_models),
                render_blurb(&servings, recipe_models, overlay),
                render_tags(
                    &m.id,
                    &servings,
                    recipe_models,
                    overlay,
                    person_names,
                    input,
                ),
            ),
            Err(e) => {
                tracing::error!(
                    meal_id = %m.id,
                    error = %e,
                    "printable: meal.servings JSON parse failed; rendering placeholder row"
                );
                (
                    format!(
                        "<div class=\"meal-title\"><span class=\"meal-icon\">⚠</span> \
                     {}</div>",
                        esc_text(&format!("(corrupt meal data, id={})", m.id))
                    ),
                    String::new(),
                    String::new(),
                )
            }
        };

    format!(
        "<div class=\"night\">\
           <div class=\"night-day\">\
             <div class=\"day-name\">{day_name}</div>\
             <div class=\"day-date\">{day_date}</div>\
             {day_tag_html}\
           </div>\
           <div class=\"night-content\">\
             {title_block}\
             {blurb}\
             {tags}\
           </div>\
         </div>",
        day_name = esc_text(&day_name),
        day_date = esc_text(&day_date),
        day_tag_html = day_tag_html,
        title_block = title_block,
        blurb = blurb,
        tags = tags,
    )
}

fn render_meal_title(
    meal_id: &str,
    servings: &[PersonServingDto],
    recipe_models: &HashMap<String, recipe::Model>,
) -> String {
    let unique_recipes = unique_recipe_ids_in_order(servings);

    if unique_recipes.is_empty() {
        // Adhoc-only meal — name from the first adhoc item, or generic fallback.
        let label = servings
            .iter()
            .find_map(|s| match s {
                PersonServingDto::Adhoc { adhoc_items, .. } => adhoc_items
                    .first()
                    .map(|i| i.name.trim())
                    .filter(|n| !n.is_empty())
                    .map(str::to_string),
                _ => None,
            })
            .unwrap_or_else(|| {
                tracing::warn!(
                    meal_id,
                    "printable: meal is adhoc-only and no adhoc item has a \
                     non-empty name; using generic 'Ad-hoc dinner' label"
                );
                "Ad-hoc dinner".to_string()
            });
        return format!(
            "<div class=\"meal-title\"><span class=\"meal-icon\">🍽</span> {}</div>",
            esc_text(&label)
        );
    }

    let primary_id = &unique_recipes[0];
    let primary = recipe_models.get(primary_id);
    let primary_name = match primary {
        Some(r) => r.name.clone(),
        None => {
            tracing::warn!(
                meal_id,
                recipe_id = %primary_id,
                "printable: meal references a recipe that no longer exists; \
                 rendering placeholder. Matches the meal_to_brief convention."
            );
            format!("(deleted recipe, id={primary_id})")
        }
    };
    let icon = primary
        .and_then(|r| r.icon.clone())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "🍽".to_string());

    let mut html = format!(
        "<div class=\"meal-title\"><span class=\"meal-icon\">{}</span> {}</div>",
        esc_text(&icon),
        esc_text(&primary_name)
    );

    if unique_recipes.len() > 1 {
        let others: Vec<String> = unique_recipes
            .iter()
            .skip(1)
            .map(|id| match recipe_models.get(id) {
                Some(r) => r.name.clone(),
                None => {
                    tracing::warn!(
                        meal_id,
                        recipe_id = %id,
                        "printable: secondary recipe reference does not resolve; \
                         rendering placeholder"
                    );
                    format!("(deleted recipe, id={id})")
                }
            })
            .collect();
        let _ = write!(
            html,
            "<div class=\"meal-also\">Also: {}</div>",
            esc_text(&others.join(", "))
        );
    }

    html
}

fn render_blurb(
    servings: &[PersonServingDto],
    recipe_models: &HashMap<String, recipe::Model>,
    overlay: Option<&DayOverlay>,
) -> String {
    let from_overlay: Option<String> = overlay.and_then(|o| o.blurb.clone());
    let text: Option<String> = from_overlay.or_else(|| {
        let primary_id = unique_recipe_ids_in_order(servings).into_iter().next()?;
        let r = recipe_models.get(&primary_id)?;
        r.description
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
    });
    match text {
        Some(t) => format!("<div class=\"meal-sub\">{}</div>", esc_text(&t)),
        None => String::new(),
    }
}

fn render_tags(
    meal_id: &str,
    servings: &[PersonServingDto],
    recipe_models: &HashMap<String, recipe::Model>,
    overlay: Option<&DayOverlay>,
    person_names: &PersonNameMap,
    input: &PrintableInput,
) -> String {
    let mut tags: Vec<String> = Vec::new();

    if let Some(time_str) = primary_total_time_label(servings, recipe_models) {
        tags.push(format!(
            "<span class=\"tag\">{}</span>",
            esc_text(&time_str)
        ));
    }

    if input.show_servings {
        for s in servings {
            let (person_id, note_opt) = match s {
                PersonServingDto::Recipe {
                    person_id, notes, ..
                } => (person_id, notes.as_deref()),
                PersonServingDto::Adhoc {
                    person_id, notes, ..
                } => (person_id, notes.as_deref()),
            };
            let Some(note) = note_opt.filter(|n| !n.trim().is_empty()) else {
                continue;
            };
            let display = if input.show_assignees {
                let name = match person_names.get(person_id) {
                    Some(n) => n.clone(),
                    None => {
                        tracing::warn!(
                            meal_id,
                            person_id,
                            "printable: meal references a person not in the \
                             active-family map (likely soft-deleted); rendering \
                             placeholder. Matches the meal_to_brief convention."
                        );
                        format!("(inactive person, id={person_id})")
                    }
                };
                format!("{name}: {note}")
            } else {
                // Trust the note text to carry the person prefix when needed
                // (the LLM idiom is "Cleo: plain gyoza" in the notes field).
                note.to_string()
            };
            tags.push(format!("<span class=\"tag\">{}</span>", esc_text(&display)));
        }
    }

    if let Some(o) = overlay {
        // prep_notes are guaranteed non-empty by the validator
        // (InputError::EmptyOverlayField). Render straight through.
        for prep in &o.prep_notes {
            tags.push(format!(
                "<span class=\"tag reminder-tag\">⚑ {}</span>",
                esc_text(prep)
            ));
        }
    }

    if tags.is_empty() {
        return String::new();
    }
    format!("<div class=\"tags-row\">{}</div>", tags.join(""))
}

fn render_reminders(items: &[DontForgetItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut html = String::from(
        "<div class=\"reminders\"><div class=\"reminders-label\">Don't<br>Forget</div>\
         <div class=\"reminders-list\">",
    );
    for item in items {
        let _ = write!(
            html,
            "<div class=\"reminder-item\"><strong>{}</strong> {}</div>",
            esc_text(&item.prefix),
            esc_text(&item.body)
        );
    }
    html.push_str("</div></div>");
    html
}

fn unique_recipe_ids_in_order(servings: &[PersonServingDto]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for s in servings {
        if let PersonServingDto::Recipe { recipe_id, .. } = s {
            if seen.insert(recipe_id.clone()) {
                out.push(recipe_id.clone());
            }
        }
    }
    out
}

fn primary_total_time_label(
    servings: &[PersonServingDto],
    recipe_models: &HashMap<String, recipe::Model>,
) -> Option<String> {
    let primary_id = unique_recipe_ids_in_order(servings).into_iter().next()?;
    let r = recipe_models.get(&primary_id)?;
    let raw = r.total_time.as_deref().filter(|s| !s.trim().is_empty())?;
    match serde_json::from_str::<TimeValueDto>(raw) {
        Ok(parsed) => Some(format!("{} {}", parsed.value, parsed.unit)),
        Err(e) => {
            tracing::warn!(
                recipe_id = %primary_id,
                error = %e,
                "printable: recipe.total_time JSON parse failed; omitting time tag"
            );
            None
        }
    }
}

fn format_week_label(start: NaiveDate, end: NaiveDate) -> String {
    if start == end {
        start.format("%b %-d, %Y").to_string()
    } else if start.year() == end.year() {
        if start.month() == end.month() {
            format!(
                "{} – {}, {}",
                start.format("%b %-d"),
                end.format("%-d"),
                end.year()
            )
        } else {
            format!(
                "{} – {}, {}",
                start.format("%b %-d"),
                end.format("%b %-d"),
                end.year()
            )
        }
    } else {
        format!(
            "{} – {}",
            start.format("%b %-d, %Y"),
            end.format("%b %-d, %Y")
        )
    }
}

/// Pluralized slot label used in the headline title and head/tab title.
/// Single-slot defaults to that slot ("Dinner", "Breakfast"); multi-slot
/// gets a generic "Meal" wrapper so the title doesn't read awkwardly
/// ("Breakfast + Dinner Plan" would force layout work this v1 doesn't do).
fn slot_label(include: &[MealType]) -> &'static str {
    match include {
        [one] => one.as_str(),
        _ => "Meal",
    }
}

/// Two-line title at the top-left of the printable. For Dinner-only weeks
/// this reads "This Week's / Dinners" (matches Steve's reference design).
/// Other single-slot variants read "This Week's / Breakfasts" etc.
/// Multi-slot widens to "This Week's / Meals" so the layout still balances.
fn title_lines(include: &[MealType]) -> (&'static str, String) {
    let bottom = match include {
        [one] => format!("{one}s"), // "Dinners", "Breakfasts", "Lunches"... acceptable for v1
        _ => "Meals".to_string(),
    };
    ("This Week's", bottom)
}

fn default_foot_note(validated: &ValidatedRange) -> String {
    let slot_summary = match validated.include.as_slice() {
        [one] => one.as_str().to_lowercase() + "s only",
        many => many
            .iter()
            .map(|s| s.as_str().to_lowercase())
            .collect::<Vec<_>>()
            .join(" + "),
    };
    format!(
        "{} · {}",
        format_week_label(validated.start, validated.end),
        slot_summary
    )
}

/// HTML-escape **text content**. Escapes `&`, `<`, `>`, `"`, `'`.
///
/// Contract: callers may only inject the output into HTML body text
/// (`<div>…</div>`, `<strong>…</strong>`) or RCDATA contexts
/// (`<title>…</title>`). Do **not** inject into attribute values, URL
/// contexts, `<script>` / `<style>` bodies, or any other parser state —
/// the 5-char escape isn't sufficient for those contexts. Every
/// substitution site in `template.html` currently honors this; if a future
/// edit moves a `{{VAR}}` into an attribute, switch to a context-aware
/// escaper instead of widening this one.
fn esc_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{IngredientAmountDto, IngredientDto};
    use crate::mcp::schemas::printable::PrintableInput;
    use chrono::Utc;

    // ─── Fixture builders ───────────────────────────────────────────

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn recipe_basic(id: &str, name: &str, icon: Option<&str>, desc: Option<&str>) -> recipe::Model {
        recipe::Model {
            id: id.into(),
            slug: name.to_lowercase().replace(' ', "-"),
            name: name.into(),
            description: desc.map(str::to_string),
            source: "manual".into(),
            source_url: None,
            parent_recipe_id: None,
            prep_time: None,
            cook_time: None,
            total_time: Some(
                serde_json::to_string(&TimeValueDto {
                    value: 25,
                    unit: "min".into(),
                })
                .unwrap(),
            ),
            total_minutes: Some(25),
            servings: 4,
            portion_size: None,
            instructions: "[]".into(),
            ingredients: "[]".into(),
            nutrition_per_serving: None,
            tags: "[]".into(),
            notes: None,
            icon: icon.map(str::to_string),
            is_favorite: false,
            times_planned: 0,
            last_planned: None,
            rating: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn meal_dinner(id: &str, d: NaiveDate, servings: Vec<PersonServingDto>) -> meal::Model {
        meal::Model {
            id: id.into(),
            date: d,
            meal_type: MealType::Dinner,
            order_index: 2,
            servings: serde_json::to_string(&servings).unwrap(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn serving_recipe(person_id: &str, recipe_id: &str, notes: Option<&str>) -> PersonServingDto {
        PersonServingDto::Recipe {
            person_id: person_id.into(),
            recipe_id: recipe_id.into(),
            servings_count: 1.0,
            notes: notes.map(str::to_string),
        }
    }

    fn person_names() -> PersonNameMap {
        let mut m = PersonNameMap::new();
        m.insert("alice".into(), "Alice".into());
        m.insert("bob".into(), "Bob".into());
        m
    }

    fn input_minimal() -> PrintableInput {
        serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-13",
        }))
        .unwrap()
    }

    // ─── Tests ──────────────────────────────────────────────────────

    #[test]
    fn render_basic_week_includes_meal_titles_and_descriptions() {
        let recipes: HashMap<String, recipe::Model> = [
            recipe_basic(
                "r-gyoza",
                "Pan-Fried Gyoza",
                Some("🥟"),
                Some("Crispy yaki-style."),
            ),
            recipe_basic(
                "r-curry",
                "Thai Green Curry",
                Some("🍛"),
                Some("Fragrant coconut curry."),
            ),
        ]
        .into_iter()
        .map(|r| (r.id.clone(), r))
        .collect();

        let meals = vec![
            meal_dinner(
                "m1",
                date(2026, 5, 11),
                vec![serving_recipe("alice", "r-gyoza", None)],
            ),
            meal_dinner(
                "m2",
                date(2026, 5, 13),
                vec![serving_recipe("alice", "r-curry", Some("Bob: extra rice"))],
            ),
        ];

        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(&meals, &recipes, &person_names(), &validated, &input);

        assert!(
            html.contains("Pan-Fried Gyoza"),
            "primary recipe title missing"
        );
        assert!(
            html.contains("Thai Green Curry"),
            "second-day recipe missing"
        );
        assert!(html.contains("🥟"), "icon should render");
        assert!(
            html.contains("Crispy yaki-style."),
            "description blurb missing"
        );
        assert!(html.contains("Bob: extra rice"), "per-person note missing");
        assert!(html.contains("25 min"), "total_time tag missing");
        // Header date label
        assert!(html.contains("May 11 – 13, 2026"), "week label missing");
        // Should NOT include the empty-state message
        assert!(!html.contains("No meals scheduled"));
    }

    #[test]
    fn render_empty_week_shows_coherent_empty_state() {
        let recipes: HashMap<String, recipe::Model> = HashMap::new();
        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(&[], &recipes, &person_names(), &validated, &input);
        assert!(html.contains("No meals scheduled"));
        assert!(html.contains("<html"));
        assert!(html.contains("create_meal"));
    }

    #[test]
    fn render_filters_to_dinner_only_by_default() {
        let recipes: HashMap<String, recipe::Model> =
            [recipe_basic("r-gyoza", "Pan-Fried Gyoza", None, None)]
                .into_iter()
                .map(|r| (r.id.clone(), r))
                .collect();

        let breakfast = meal::Model {
            id: "mb".into(),
            date: date(2026, 5, 11),
            meal_type: MealType::Breakfast,
            order_index: 0,
            servings: serde_json::to_string(&vec![serving_recipe("alice", "r-gyoza", None)])
                .unwrap(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let dinner = meal_dinner(
            "md",
            date(2026, 5, 11),
            vec![serving_recipe("alice", "r-gyoza", None)],
        );

        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(
            &[breakfast.clone(), dinner],
            &recipes,
            &person_names(),
            &validated,
            &input,
        );
        // One row for the dinner, none for the breakfast.
        assert_eq!(html.matches("class=\"night\"").count(), 1);
    }

    #[test]
    fn render_html_escapes_recipe_name_and_notes() {
        let mut nasty = recipe_basic("r-x", "Steve's <script>alert(1)</script>", None, None);
        nasty.description = Some("Use \"caution\" & cleanup.".into());
        let recipes: HashMap<String, recipe::Model> =
            [nasty].into_iter().map(|r| (r.id.clone(), r)).collect();

        let meals = vec![meal_dinner(
            "m1",
            date(2026, 5, 11),
            vec![serving_recipe("alice", "r-x", Some("<b>bold</b>"))],
        )];

        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(&meals, &recipes, &person_names(), &validated, &input);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&quot;caution&quot;"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"));
    }

    #[test]
    fn render_multi_recipe_meal_shows_primary_plus_also_secondary() {
        let recipes: HashMap<String, recipe::Model> = [
            recipe_basic("r-gyoza", "Pan-Fried Gyoza", None, None),
            recipe_basic("r-curry", "Thai Green Curry", None, None),
        ]
        .into_iter()
        .map(|r| (r.id.clone(), r))
        .collect();

        let meals = vec![meal_dinner(
            "m1",
            date(2026, 5, 11),
            vec![
                serving_recipe("alice", "r-gyoza", None),
                serving_recipe("bob", "r-curry", None),
            ],
        )];
        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(&meals, &recipes, &person_names(), &validated, &input);
        // Primary appears in the meal-title; secondary in the meal-also line.
        assert!(html.contains("Pan-Fried Gyoza"));
        assert!(html.contains("meal-also"));
        assert!(html.contains("Also: Thai Green Curry"));
    }

    #[test]
    fn render_overlay_blurb_overrides_recipe_description() {
        let recipes: HashMap<String, recipe::Model> = [recipe_basic(
            "r-gyoza",
            "Pan-Fried Gyoza",
            None,
            Some("Stored recipe description."),
        )]
        .into_iter()
        .map(|r| (r.id.clone(), r))
        .collect();

        let meals = vec![meal_dinner(
            "m1",
            date(2026, 5, 11),
            vec![serving_recipe("alice", "r-gyoza", None)],
        )];

        let input: PrintableInput = serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-13",
            "day_overlays": [{
                "date": "2026-05-11",
                "tag": "Time Crunch",
                "blurb": "Override blurb for the week.",
                "prep_notes": ["Marinate morning of"]
            }]
        }))
        .unwrap();
        let validated = input.validate().unwrap();
        let html = render(&meals, &recipes, &person_names(), &validated, &input);
        assert!(html.contains("Override blurb for the week."));
        assert!(!html.contains("Stored recipe description."));
        assert!(html.contains("Time Crunch"));
        assert!(html.contains("Marinate morning of"));
        assert!(html.contains("reminder-tag"));
    }

    #[test]
    fn render_drops_overlays_outside_range() {
        let recipes: HashMap<String, recipe::Model> =
            [recipe_basic("r-gyoza", "Pan-Fried Gyoza", None, None)]
                .into_iter()
                .map(|r| (r.id.clone(), r))
                .collect();

        let meals = vec![meal_dinner(
            "m1",
            date(2026, 5, 11),
            vec![serving_recipe("alice", "r-gyoza", None)],
        )];

        let input: PrintableInput = serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-13",
            "day_overlays": [{
                "date": "2026-05-30",
                "tag": "Should not appear",
                "blurb": "stale overlay from a different week"
            }]
        }))
        .unwrap();
        let validated = input.validate().unwrap();
        let html = render(&meals, &recipes, &person_names(), &validated, &input);
        assert!(!html.contains("Should not appear"));
        assert!(!html.contains("stale overlay"));
    }

    #[test]
    fn render_dont_forget_renders_dark_block_with_prefix_bolded() {
        let recipes = HashMap::new();
        let input: PrintableInput = serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-13",
            "dont_forget": [
                {"prefix": "Wed night:", "body": "cook extra rice"},
                {"prefix": "Fri morning:", "body": "marinate ribeye"}
            ]
        }))
        .unwrap();
        let validated = input.validate().unwrap();
        let html = render(&[], &recipes, &person_names(), &validated, &input);
        assert!(html.contains("class=\"reminders\""));
        assert!(html.contains("<strong>Wed night:</strong>"));
        assert!(html.contains("<strong>Fri morning:</strong>"));
        assert!(html.contains("cook extra rice"));
    }

    #[test]
    fn render_week_theme_and_use_up_notes_appear_in_header() {
        let recipes = HashMap::new();
        let input: PrintableInput = serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-13",
            "week_theme": "Freezer Clear",
            "use_up_notes": ["Frozen gyoza → Monday", "Chicken thighs → Tuesday"]
        }))
        .unwrap();
        let validated = input.validate().unwrap();
        let html = render(&[], &recipes, &person_names(), &validated, &input);
        assert!(html.contains("class=\"badge\""));
        assert!(html.contains("Freezer Clear"));
        assert!(html.contains("Frozen gyoza → Monday"));
        assert!(html.contains("Chicken thighs → Tuesday"));
    }

    #[test]
    fn render_default_foot_note_includes_date_range_and_slot_summary() {
        let recipes = HashMap::new();
        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(&[], &recipes, &person_names(), &validated, &input);
        // Default footer follows the predictability format: "{date range} · {slots}"
        assert!(html.contains("May 11 – 13, 2026 · dinners only"));
    }

    #[test]
    fn render_explicit_foot_note_overrides_default() {
        let recipes = HashMap::new();
        let input: PrintableInput = serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-13",
            "foot_note": "Back-friendly week · Hot weather menu"
        }))
        .unwrap();
        let validated = input.validate().unwrap();
        let html = render(&[], &recipes, &person_names(), &validated, &input);
        assert!(html.contains("Back-friendly week · Hot weather menu"));
        // Should not double up with the default predictability footer.
        assert!(!html.contains("dinners only"));
    }

    #[test]
    fn render_adhoc_only_meal_falls_back_to_first_item_name() {
        let recipes = HashMap::new();
        let adhoc = PersonServingDto::Adhoc {
            person_id: "alice".into(),
            adhoc_items: vec![IngredientDto {
                name: "leftover pizza".into(),
                prep: None,
                amount: IngredientAmountDto::Single { value: 1.0 },
                unit: "slice".into(),
                notes: None,
                or_alternative: None,
            }],
            notes: None,
        };
        let meals = vec![meal_dinner("m1", date(2026, 5, 11), vec![adhoc])];
        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(&meals, &recipes, &person_names(), &validated, &input);
        assert!(html.contains("leftover pizza"));
    }

    #[test]
    fn render_uses_fallback_icon_when_recipe_has_none() {
        let recipes: HashMap<String, recipe::Model> =
            [recipe_basic("r-x", "Carbonara", None, None)]
                .into_iter()
                .map(|r| (r.id.clone(), r))
                .collect();
        let meals = vec![meal_dinner(
            "m1",
            date(2026, 5, 11),
            vec![serving_recipe("alice", "r-x", None)],
        )];
        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(&meals, &recipes, &person_names(), &validated, &input);
        assert!(
            html.contains("🍽"),
            "fallback icon should render when icon is None"
        );
    }

    #[test]
    fn render_includes_page_break_avoid_rules_for_print_fit() {
        // The single-page constraint is load-bearing; pin the print CSS
        // rules so an accidental template edit can't silently regress fit.
        let recipes = HashMap::new();
        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(&[], &recipes, &person_names(), &validated, &input);
        assert!(html.contains("@page"));
        assert!(html.contains("size: letter portrait"));
        assert!(html.contains("page-break-inside: avoid"));
        assert!(html.contains("-webkit-line-clamp"));
    }

    // ─── New tests for code-review fixes ──────────────────────────────

    #[test]
    fn render_title_adapts_to_breakfast_only_include() {
        // Hardcoded "Dinners" was a bug — `include` should drive the
        // title. Pin both the headline and the head/tab title.
        let input: PrintableInput = serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-13",
            "include": ["breakfast"]
        }))
        .unwrap();
        let validated = input.validate().unwrap();
        let html = render(&[], &HashMap::new(), &person_names(), &validated, &input);
        assert!(
            html.contains("<title>Breakfast Plan — "),
            "head title should reflect slot: {}",
            &html[..200]
        );
        assert!(
            html.contains(">Breakfasts<"),
            "headline title should reflect slot"
        );
        assert!(
            !html.contains(">Dinners<"),
            "Dinners must not appear for a breakfast-only week"
        );
    }

    #[test]
    fn render_title_widens_to_meals_when_multiple_slots_included() {
        let input: PrintableInput = serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-13",
            "include": ["breakfast", "dinner"]
        }))
        .unwrap();
        let validated = input.validate().unwrap();
        let html = render(&[], &HashMap::new(), &person_names(), &validated, &input);
        assert!(html.contains("<title>Meal Plan — "));
        assert!(html.contains(">Meals<"));
    }

    #[test]
    fn render_corrupt_servings_json_renders_placeholder_row_and_continues() {
        // A single corrupt row should NOT fail the whole printable —
        // family still gets the other days. Verified by checking that
        // the second day's recipe still renders alongside the placeholder.
        let recipes: HashMap<String, recipe::Model> =
            [recipe_basic("r-gyoza", "Pan-Fried Gyoza", None, None)]
                .into_iter()
                .map(|r| (r.id.clone(), r))
                .collect();
        let corrupt = meal::Model {
            id: "m-bad".into(),
            date: date(2026, 5, 11),
            meal_type: MealType::Dinner,
            order_index: 2,
            servings: "{not valid json".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let good = meal_dinner(
            "m-ok",
            date(2026, 5, 12),
            vec![serving_recipe("alice", "r-gyoza", None)],
        );
        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(
            &[corrupt, good],
            &recipes,
            &person_names(),
            &validated,
            &input,
        );
        assert!(html.contains("(corrupt meal data, id=m-bad)"));
        assert!(html.contains("Pan-Fried Gyoza"));
        assert_eq!(html.matches("class=\"night\"").count(), 2);
    }

    #[test]
    fn render_deleted_recipe_reference_uses_placeholder_consistent_with_meal_to_brief() {
        // No recipes in the map; meal references "r-ghost" → placeholder
        // matches the meal_to_brief wording so log searches can grep both.
        let meals = vec![meal_dinner(
            "m1",
            date(2026, 5, 11),
            vec![serving_recipe("alice", "r-ghost", None)],
        )];
        let input = input_minimal();
        let validated = input.validate().unwrap();
        let html = render(&meals, &HashMap::new(), &person_names(), &validated, &input);
        assert!(html.contains("(deleted recipe, id=r-ghost)"));
    }

    #[test]
    fn render_user_supplied_value_containing_placeholder_token_does_not_get_re_substituted() {
        // Regression guard for the chained-replace bug Copilot flagged: a
        // user-supplied overlay value containing a literal `{{NAME}}` token
        // must NOT be picked up by a later substitution pass. Single-pass
        // walker means inserted values are never re-scanned.
        let recipes = HashMap::new();
        let input: PrintableInput = serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-13",
            // Adversarial value — if the renderer chained .replace() calls,
            // this would inject the nights HTML (or empty-state markup) into
            // the badge.
            "week_theme": "{{NIGHTS_OR_EMPTY}}",
            "foot_note": "{{REMINDERS_BLOCK}}"
        }))
        .unwrap();
        let validated = input.validate().unwrap();
        let html = render(&[], &recipes, &person_names(), &validated, &input);
        // The literal placeholder text must survive verbatim in the badge.
        assert!(
            html.contains(">{{NIGHTS_OR_EMPTY}}<"),
            "user-supplied placeholder string should render as literal text \
             inside the badge, not get re-substituted; html: {html}"
        );
        // The foot note also lands escaped/intact.
        assert!(html.contains(">{{REMINDERS_BLOCK}}<"));
        // And the genuine empty-state still rendered into the real slot.
        assert!(html.contains("No meals scheduled"));
    }

    #[test]
    fn substitute_walks_template_in_one_pass_and_never_rescans_insertions() {
        // Unit-level pin for the single-pass walker.
        let y = "{{Y}}";
        let real_y = "REAL_Y";
        let out = super::substitute("a {{X}} b {{Y}} c", &[("X", y), ("Y", real_y)]);
        // `{{Y}}` produced by the X substitution must NOT be picked up by
        // the subsequent Y substitution — it survives as literal text.
        assert_eq!(out, "a {{Y}} b REAL_Y c");
    }

    #[test]
    fn substitute_unknown_placeholder_emits_verbatim_with_braces() {
        // Unknown placeholder = bug in the template (or missing key in the
        // replacements slice). Emit verbatim so the bug is visible in the
        // rendered output rather than silently dropping content.
        let out = super::substitute("hello {{UNKNOWN}} world", &[]);
        assert_eq!(out, "hello {{UNKNOWN}} world");
    }

    #[test]
    fn substitute_unclosed_placeholder_emits_remainder_verbatim() {
        // Defensive: an unclosed `{{` in the template would otherwise
        // truncate or panic. Emit verbatim so the bug is visible.
        let v = "VALUE";
        let out = super::substitute("hello {{UNCLOSED rest", &[("X", v)]);
        assert_eq!(out, "hello {{UNCLOSED rest");
    }

    #[test]
    fn render_soft_deleted_person_uses_placeholder_when_show_assignees_true() {
        // person_names is empty; meal serving references "ghost". With
        // show_assignees on, the per-serving tag includes the placeholder.
        let recipes: HashMap<String, recipe::Model> =
            [recipe_basic("r-x", "Carbonara", None, None)]
                .into_iter()
                .map(|r| (r.id.clone(), r))
                .collect();
        let meals = vec![meal_dinner(
            "m1",
            date(2026, 5, 11),
            vec![serving_recipe("ghost", "r-x", Some("plain pasta"))],
        )];
        let input: PrintableInput = serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-13",
            "show_assignees": true
        }))
        .unwrap();
        let validated = input.validate().unwrap();
        let html = render(&meals, &recipes, &PersonNameMap::new(), &validated, &input);
        assert!(html.contains("(inactive person, id=ghost): plain pasta"));
    }

    /// Manual single-page-fit verification helper. CSS rules pinned by the
    /// other tests are necessary but not sufficient — the only way to truly
    /// confirm a maxed-out week renders on one US Letter sheet is to open
    /// the output in a browser and look. This test dumps a realistic
    /// 7-day fixture with every overlay field exercised to
    /// `/tmp/fewd-printable-sample.html`, then anyone can run:
    ///
    ///   `cargo test --lib dump_max_content_sample -- --ignored --nocapture`
    ///
    /// and inspect the file in print preview. `#[ignore]` keeps it out of
    /// the default CI/test run so the file write isn't repeated on every
    /// invocation of `cargo test`.
    #[test]
    #[ignore = "writes /tmp/fewd-printable-sample.html for manual print-preview inspection"]
    fn dump_max_content_sample() {
        use std::fs;

        let recipes: HashMap<String, recipe::Model> = vec![
            recipe_basic(
                "r-gyoza",
                "Pan-Fried Gyoza + Rice",
                Some("🥟"),
                Some("Crispy yaki-style gyoza with soy-vinegar dipping sauce. Done in 15 min."),
            ),
            recipe_basic(
                "r-gnocchi",
                "Gnocchi w/ Sausage & Cream",
                Some("🍝"),
                Some("Spicy chicken sausage, white wine cream. Amanda's kind of recipe."),
            ),
            recipe_basic(
                "r-bake",
                "Breville Breaded Chicken + Rice",
                Some("🍗"),
                Some("400°F bake, 22 min, flip at 12. Almost entirely hands-off."),
            ),
            recipe_basic(
                "r-teri",
                "Chicken Teriyaki Bowls",
                Some("🍱"),
                Some("Quick weeknight bowl. Steve + kids, Amanda at book club."),
            ),
            recipe_basic(
                "r-ribeye",
                "Korean BBQ Ribeye — Bulgogi-Style",
                Some("🥩"),
                Some("Weber Summit. Thin-sliced ribeye, sweet-savory marinade."),
            ),
            recipe_basic(
                "r-smash",
                "Smash Burgers + Fries",
                Some("🍔"),
                Some("Double-stacked crispy-edge smash burgers. Fries in the Breville."),
            ),
            recipe_basic(
                "r-fried",
                "Bacon Fried Rice — Nikumaki Style",
                Some("🍳"),
                Some("Sweet-umami glaze, crispy bacon, restaurant feel. Sunday project."),
            ),
        ]
        .into_iter()
        .map(|r| (r.id.clone(), r))
        .collect();

        let meals: Vec<meal::Model> = [
            ("m1", date(2026, 5, 11), "r-gyoza"),
            ("m2", date(2026, 5, 12), "r-gnocchi"),
            ("m3", date(2026, 5, 13), "r-bake"),
            ("m4", date(2026, 5, 14), "r-teri"),
            ("m5", date(2026, 5, 15), "r-ribeye"),
            ("m6", date(2026, 5, 16), "r-smash"),
            ("m7", date(2026, 5, 17), "r-fried"),
        ]
        .into_iter()
        .map(|(id, d, recipe_id)| {
            meal_dinner(
                id,
                d,
                vec![
                    serving_recipe("alice", recipe_id, Some("Cleo: plain rice on the side")),
                    serving_recipe("bob", recipe_id, Some("Viv: no parm")),
                ],
            )
        })
        .collect();

        let input: PrintableInput = serde_json::from_value(serde_json::json!({
            "start_date": "2026-05-11",
            "end_date": "2026-05-17",
            "week_theme": "Freezer Clear",
            "use_up_notes": [
                "Frozen gyoza → Monday",
                "Spicy chicken sausage → Tuesday",
                "Breaded chicken breasts → Wednesday",
            ],
            "dont_forget": [
                {"prefix": "Wed night:", "body": "cook extra chipotle rice and refrigerate — Sunday's fried rice MUST use cold day-old rice"},
                {"prefix": "Fri morning:", "body": "get the bulgogi ribeye into marinade before noon"},
                {"prefix": "Mon:", "body": "Girl Scouts — gyoza ready by 7:15, Cleo shower by 8pm"},
                {"prefix": "Thu:", "body": "Amanda leaves for book club at 6 — Steve and girls on their own"},
            ],
            "day_overlays": [
                {"date": "2026-05-11", "tag": "Time Crunch", "prep_notes": ["Girls Scouts tonight"]},
                {"date": "2026-05-13", "tag": "Steve Cooks", "prep_notes": ["⚑ Make extra rice"]},
                {"date": "2026-05-14", "tag": "Steve + Girls"},
                {"date": "2026-05-15", "prep_notes": ["⚑ Marinate morning of — needs 2–8 hrs"]},
                {"date": "2026-05-17", "prep_notes": ["⚑ Must use cold rice from Wed"]},
            ],
            "foot_note": "Back-friendly week · Hot weather menu · Freezer cleared ✓"
        }))
        .unwrap();
        let validated = input.validate().unwrap();
        let html = render(&meals, &recipes, &person_names(), &validated, &input);

        let path = "/tmp/fewd-printable-sample.html";
        fs::write(path, &html).expect("write sample HTML");
        eprintln!(
            "\nWrote {} ({} bytes). Open it in a browser and use print preview to verify single-page fit.\n",
            path,
            html.len()
        );
    }
}
