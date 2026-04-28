use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ingredient_amount::{is_known_unit, try_parse_amount_json};
use crate::ingredient_splitter::split_name_and_prep;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Walks every recipe and repairs ingredient rows misbucketed by the old
/// `recipe_parser::parse_ingredient_line` (pre-fewd-4i3). Detects four
/// patterns surfaced by the dietpi audit on 2026-04-27 and rewrites the
/// JSON shape in place; rows that don't match any pattern pass through.
///
/// Patterns:
///
/// - **A — comma-tail unit.** `unit.ends_with(',')`. Caused by
///   `splitn(3, ' ')` putting `"zucchini,"` at parts[1] when parts[1] was
///   actually part of the name. Repair: merge unit into name with the
///   comma preserved, run the splitter to peel any prep clause, set
///   `unit = "whole"`.
///
/// - **B — em/en-dash range left in name.** `name` matches
///   `\d+[-–—]\d+ ` (leading range token), `unit == "to taste"`,
///   `amount == Single { 1.0 }`. Caused by `try_parse_amount` rejecting
///   Unicode dashes. Repair: re-tokenize the name with the fixed
///   parser logic.
///
/// - **C — Unicode vulgar fraction left in name.** `name` starts with a
///   vulgar fraction OR `<digit><vulgar fraction>`, same failed-parse
///   signature. Repair: same as B.
///
/// - **E — swapped name/unit.** `is_known_unit(name) && !is_known_unit(unit)
///   && !unit.is_empty()`. Catches the post-fewd-xez state of rows like
///   `name="leaves", unit="bay"` (originally `2 bay leaves`) and
///   `name="stalks", prep="finely diced", unit="celery"` (originally
///   `2 celery stalks, finely diced` — m13 already peeled the prep).
///   Repair: reconstruct `name = "{unit} {name}"`, run splitter, set
///   `unit = "whole"`.
///
/// Scoped to `recipes` only — the dietpi prod audit confirmed
/// drink_recipes / meals.servings / meal_templates.servings carry zero
/// ingredient data. The fewd-4i3-fixed parser is wired into the runtime
/// ingest paths (markdown import, URL importer, MCP boundary), so future
/// writes won't need backfilling.
///
/// Idempotent: re-parsing a fixed row produces the same fixed row. JSON
/// parse errors halt the migration loudly with the recipe id (mirrors
/// m13's contract — silent skip would hide corrupt data).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id, ingredients FROM recipes ORDER BY id".to_string(),
            ))
            .await?;

        for row in rows {
            let id: String = row.try_get("", "id")?;
            let original: String = row.try_get("", "ingredients")?;

            let rewritten = rewrite_ingredients_json(&original).map_err(|e| {
                DbErr::Custom(format!(
                    "recipe {id} has unprocessable ingredients JSON: {e}. \
                     Fix the row manually before re-running migrations \
                     (the reparse will not silently skip corrupt data)."
                ))
            })?;

            let Some(rewritten) = rewritten else {
                continue;
            };

            db.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE recipes SET ingredients = ? WHERE id = ?".to_string(),
                [rewritten.into(), id.into()],
            ))
            .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op. Reverting a repaired row would require fabricating the
        // original misparsed shape — there's no value in re-corrupting.
        Ok(())
    }
}

/// Frozen-in-time copy of the ingredient shape (m14 owns its own struct
/// per the project convention; do not share with m13 even though the
/// shapes are currently identical).
#[derive(Debug, Deserialize, Serialize)]
struct Ingredient {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prep: Option<String>,
    amount: Value,
    /// `unit` is consistently a string in prod (see m13 audit). Migration
    /// inspects it directly rather than as opaque Value so we can do the
    /// "ends_with(',')" / `is_known_unit` checks without a serde dance.
    #[serde(default)]
    unit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

fn rewrite_ingredients_json(raw: &str) -> Result<Option<String>, serde_json::Error> {
    let mut ingredients: Vec<Ingredient> = serde_json::from_str(raw)?;

    let mut changed = false;
    for ing in ingredients.iter_mut() {
        if reparse_ingredient(ing) {
            changed = true;
        }
    }

    if !changed {
        return Ok(None);
    }
    Ok(Some(serde_json::to_string(&ingredients)?))
}

/// Apply the first matching pattern to `ing` in place. Returns `true` if
/// any field changed. Patterns are mutually exclusive in practice — a row
/// with a comma-tail unit doesn't also have an em-dash range in `name` —
/// but we check them in priority order: A → B/C → E.
fn reparse_ingredient(ing: &mut Ingredient) -> bool {
    if try_pattern_a(ing) {
        return true;
    }
    if try_pattern_b_or_c(ing) {
        return true;
    }
    if try_pattern_e(ing) {
        return true;
    }
    false
}

/// Pattern A: `unit.ends_with(',')`. Reconstruct the comma'd name + prep
/// pair and run the splitter.
fn try_pattern_a(ing: &mut Ingredient) -> bool {
    if !ing.unit.ends_with(',') {
        return false;
    }
    let raw_name = format!("{} {}", ing.unit, ing.name);
    let (name, prep) = split_name_and_prep(&raw_name);
    ing.name = name;
    // Pattern A only sets prep if the splitter found one. If the original
    // row had a prep already (unlikely for misparses) we'd merge — but the
    // fewd-xez splitter is idempotent, so feeding it a comma'd name yields
    // the right answer either way.
    if prep.is_some() {
        ing.prep = prep;
    }
    ing.unit = "whole".to_string();
    true
}

/// Pattern B/C: name leads with an em/en-dash range OR a Unicode vulgar
/// fraction; failed-parse signature on amount + unit. Re-tokenize the name
/// using the same logic the runtime parser's 3-part branch applies.
fn try_pattern_b_or_c(ing: &mut Ingredient) -> bool {
    if !is_failed_parse(&ing.amount, &ing.unit) {
        return false;
    }
    if !name_starts_with_failed_amount(&ing.name) {
        return false;
    }

    // Re-tokenize: splitn(3, ' '). Materialize as owned strings so we can
    // mutate `ing.name` after.
    let parts: Vec<String> = ing.name.splitn(3, ' ').map(|s| s.to_string()).collect();
    if parts.len() < 2 {
        return false;
    }
    let Some(new_amount) = try_parse_amount_json(&parts[0]) else {
        return false;
    };

    if parts.len() == 2 {
        // "12–15 leaves" — amount + (name | unit-only). Treat parts[1] as
        // the new name with unit=whole.
        ing.amount = new_amount;
        ing.name = parts[1].clone();
        ing.unit = "whole".to_string();
        return true;
    }

    // 3 parts.
    let token = &parts[1];
    let token_stripped = token.strip_suffix(',').unwrap_or(token);
    if is_known_unit(token_stripped) {
        ing.amount = new_amount;
        let (split_name, split_prep) = split_name_and_prep(&parts[2]);
        ing.name = split_name;
        if split_prep.is_some() {
            ing.prep = split_prep;
        }
        ing.unit = token_stripped.to_string();
    } else {
        let raw_name = format!("{} {}", token, parts[2]);
        let (split_name, split_prep) = split_name_and_prep(&raw_name);
        ing.amount = new_amount;
        ing.name = split_name;
        if split_prep.is_some() {
            ing.prep = split_prep;
        }
        ing.unit = "whole".to_string();
    }
    true
}

/// Pattern E: name is a known unit, unit is not. Caused by post-fewd-xez
/// state of rows like `2 bay leaves` or `2 celery stalks, finely diced`
/// (m13 already peeled the prep so the comma signal is gone).
fn try_pattern_e(ing: &mut Ingredient) -> bool {
    if ing.unit.is_empty() {
        return false;
    }
    if !is_known_unit(&ing.name) {
        return false;
    }
    if is_known_unit(&ing.unit) {
        return false;
    }
    let raw_name = format!("{} {}", ing.unit, ing.name);
    let (name, prep) = split_name_and_prep(&raw_name);
    ing.name = name;
    if prep.is_some() {
        ing.prep = prep;
    }
    ing.unit = "whole".to_string();
    true
}

/// Detects the failed-parse signature: `amount = Single { 1.0 }` AND
/// `unit = "to taste"`. The runtime parser's "no parseable amount" branch
/// produces exactly this shape — so we know any row with a leading
/// dash-range or vulgar-fraction in the name AND this signature is
/// recoverable.
fn is_failed_parse(amount: &Value, unit: &str) -> bool {
    if unit != "to taste" {
        return false;
    }
    let Some(kind) = amount.get("type").and_then(|t| t.as_str()) else {
        return false;
    };
    if kind != "single" {
        return false;
    }
    let value = amount.get("value").and_then(|v| v.as_f64());
    matches!(value, Some(v) if (v - 1.0).abs() < 1e-9)
}

fn name_starts_with_failed_amount(name: &str) -> bool {
    let trimmed = name.trim_start();
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    // Em/en-dash range: leading digit, then digits, then a dash variant,
    // then digits, then a space.
    if first.is_ascii_digit() {
        // Walk past leading digits.
        let mut idx = first.len_utf8();
        for c in chars.clone() {
            if c.is_ascii_digit() {
                idx += c.len_utf8();
            } else {
                break;
            }
        }
        let after_int = &trimmed[idx..];
        let mut after_chars = after_int.chars();
        let Some(maybe_dash) = after_chars.next() else {
            return false;
        };
        if matches!(maybe_dash, '-' | '–' | '—') {
            // Followed by another digit?
            return after_chars.next().is_some_and(|c| c.is_ascii_digit());
        }
        // Mixed unicode form like "1½ cups milk".
        if is_vulgar_fraction(maybe_dash) {
            return true;
        }
    }

    // Standalone vulgar fraction: `"¼ teaspoon salt"`.
    is_vulgar_fraction(first)
}

fn is_vulgar_fraction(c: char) -> bool {
    matches!(
        c,
        '¼' | '½'
            | '¾'
            | '⅓'
            | '⅔'
            | '⅕'
            | '⅖'
            | '⅗'
            | '⅘'
            | '⅙'
            | '⅚'
            | '⅛'
            | '⅜'
            | '⅝'
            | '⅞'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database};
    use serde_json::json;

    const RECIPES_SCHEMA: &str = r#"
        CREATE TABLE recipes (
            id TEXT NOT NULL PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            slug TEXT NOT NULL DEFAULT '',
            servings INTEGER NOT NULL DEFAULT 1,
            source TEXT NOT NULL DEFAULT 'manual',
            instructions TEXT NOT NULL DEFAULT '',
            ingredients TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]',
            is_favorite INTEGER NOT NULL DEFAULT 0,
            times_made INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT '2026-01-01',
            updated_at TEXT NOT NULL DEFAULT '2026-01-01'
        );
    "#;

    async fn legacy_db() -> sea_orm_migration::sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(RECIPES_SCHEMA).await.unwrap();
        db
    }

    async fn insert(db: &sea_orm_migration::sea_orm::DatabaseConnection, id: &str, json: &str) {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO recipes (id, ingredients) VALUES (?, ?)".to_string(),
            [id.into(), json.into()],
        ))
        .await
        .unwrap();
    }

    async fn ingredients(
        db: &sea_orm_migration::sea_orm::DatabaseConnection,
        id: &str,
    ) -> Vec<Ingredient> {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT ingredients FROM recipes WHERE id = ?".to_string(),
                [id.into()],
            ))
            .await
            .unwrap()
            .unwrap();
        let raw: String = row.try_get("", "ingredients").unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    fn ing_json(name: &str, prep: Option<&str>, amount: Value, unit: &str) -> Value {
        let mut v = json!({
            "name": name,
            "amount": amount,
            "unit": unit,
            "notes": null,
        });
        if let Some(p) = prep {
            v["prep"] = json!(p);
        }
        v
    }

    fn single(value: f64) -> Value {
        json!({ "type": "single", "value": value })
    }

    fn range(min: f64, max: f64) -> Value {
        json!({ "type": "range", "min": min, "max": max })
    }

    fn array_one(ing: Value) -> String {
        serde_json::to_string(&[ing]).unwrap()
    }

    // ─── Pattern A — comma-tail unit ──────────────────────────────────

    #[tokio::test]
    async fn pattern_a_zucchini_comma_unit() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "sliced into half-moons",
                None,
                single(1.0),
                "zucchini,",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "zucchini");
        assert_eq!(after[0].prep.as_deref(), Some("sliced into half-moons"));
        assert_eq!(after[0].unit, "whole");
    }

    #[tokio::test]
    async fn pattern_a_scallions_comma_unit() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json("thinly sliced", None, single(1.0), "scallions,")),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "scallions");
        assert_eq!(after[0].prep.as_deref(), Some("thinly sliced"));
        assert_eq!(after[0].unit, "whole");
    }

    // ─── Pattern B — em/en-dash range left in name ────────────────────

    #[tokio::test]
    async fn pattern_b_en_dash_sage_leaves() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "12–15 fresh sage leaves",
                None,
                single(1.0),
                "to taste",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        // "fresh" is not a known unit, so the parser's 3-part branch (which
        // we mirror) merges it into the name with unit=whole.
        assert_eq!(after[0].name, "fresh sage leaves");
        assert_eq!(after[0].unit, "whole");
        assert_eq!(after[0].amount, range(12.0, 15.0));
    }

    #[tokio::test]
    async fn pattern_b_em_dash_lbs_short_ribs() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "3–4 lbs bone-in beef short ribs",
                None,
                single(1.0),
                "to taste",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "bone-in beef short ribs");
        assert_eq!(after[0].unit, "lbs");
        assert_eq!(after[0].amount, range(3.0, 4.0));
    }

    #[tokio::test]
    async fn pattern_b_balls_pizza_dough() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "2–3 balls store-bought pizza dough",
                None,
                single(1.0),
                "to taste",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "store-bought pizza dough");
        assert_eq!(after[0].unit, "balls");
        assert_eq!(after[0].amount, range(2.0, 3.0));
    }

    // ─── Pattern C — Unicode vulgar fraction in name ──────────────────

    #[tokio::test]
    async fn pattern_c_quarter_teaspoon_salt() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json("¼ teaspoon salt", None, single(1.0), "to taste")),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "salt");
        assert_eq!(after[0].unit, "teaspoon");
        assert_eq!(after[0].amount, single(0.25));
    }

    #[tokio::test]
    async fn pattern_c_three_quarters_cup_flour() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "¾ cup all-purpose flour",
                None,
                single(1.0),
                "to taste",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "all-purpose flour");
        assert_eq!(after[0].unit, "cup");
        assert_eq!(after[0].amount, single(0.75));
    }

    #[tokio::test]
    async fn pattern_c_mixed_fraction_milk() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json("1½ cups milk", None, single(1.0), "to taste")),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "milk");
        assert_eq!(after[0].unit, "cups");
        assert_eq!(after[0].amount, single(1.5));
    }

    // ─── Pattern E — swapped name/unit (post-m13 state) ───────────────

    #[tokio::test]
    async fn pattern_e_bay_leaves() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json("leaves", None, single(2.0), "bay")),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "bay leaves");
        assert_eq!(after[0].unit, "whole");
        assert_eq!(after[0].amount, single(2.0));
    }

    #[tokio::test]
    async fn pattern_e_celery_stalks_with_existing_prep() {
        // Post-m13 state: m13 already peeled "finely diced" into prep
        // when it ran on the original misparse. Pattern E reconstructs the
        // name without disturbing the existing prep.
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "stalks",
                Some("finely diced"),
                single(2.0),
                "celery",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "celery stalks");
        assert_eq!(after[0].prep.as_deref(), Some("finely diced"));
        assert_eq!(after[0].unit, "whole");
    }

    // ─── Idempotency + pass-through ───────────────────────────────────

    #[tokio::test]
    async fn idempotent_on_rerun() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "12–15 fresh sage leaves",
                None,
                single(1.0),
                "to taste",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after_first = ingredients(&db, "r1").await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after_second = ingredients(&db, "r1").await;
        assert_eq!(after_first[0].name, after_second[0].name);
        assert_eq!(after_first[0].amount, after_second[0].amount);
        assert_eq!(after_first[0].unit, after_second[0].unit);
    }

    #[tokio::test]
    async fn passes_through_clean_rows() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json("garlic", Some("minced"), single(3.0), "cloves")),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "garlic");
        assert_eq!(after[0].prep.as_deref(), Some("minced"));
        assert_eq!(after[0].unit, "cloves");
        assert_eq!(after[0].amount, single(3.0));
    }

    #[tokio::test]
    async fn passes_through_80_20_ground_beef() {
        // The `80/20 ground beef` row has amount=4.0, unit="to taste",
        // name="ground beef". No reliable round-trip — leave alone.
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json("ground beef", None, single(4.0), "to taste")),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "ground beef");
        assert_eq!(after[0].unit, "to taste");
        assert_eq!(after[0].amount, single(4.0));
    }

    #[tokio::test]
    async fn passes_through_empty_unit_rows() {
        // Rows like `name="medium red onions", unit=""` were stored that
        // way directly (not via the parser's failed-parse path). Pattern E
        // requires non-empty unit; nothing else matches. Leave alone.
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json("medium red onions", None, single(4.0), "")),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "medium red onions");
        assert_eq!(after[0].unit, "");
    }

    #[tokio::test]
    async fn passes_through_cans_with_unit_whole() {
        // The `2 cans (28 oz total)` row — author dropped the noun. Stored
        // as name="cans", unit="whole" (post-extract_notes). No fix possible.
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json("cans", None, single(2.0), "whole")),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "cans");
        assert_eq!(after[0].unit, "whole");
    }

    // ─── Multi-ingredient row + selective UPDATE ──────────────────────

    #[tokio::test]
    async fn mixed_recipe_only_updates_changed_ingredients() {
        let db = legacy_db().await;
        let mixed = serde_json::to_string(&[
            ing_json("garlic", Some("minced"), single(3.0), "cloves"), // clean
            ing_json("sliced into half-moons", None, single(1.0), "zucchini,"), // pattern A
            ing_json("salt", None, single(1.0), "tsp"),                // clean
            ing_json("¼ teaspoon salt", None, single(1.0), "to taste"), // pattern C
        ])
        .unwrap();
        insert(&db, "r1", &mixed).await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        // Index 0: clean
        assert_eq!(after[0].name, "garlic");
        // Index 1: pattern A
        assert_eq!(after[1].name, "zucchini");
        assert_eq!(after[1].prep.as_deref(), Some("sliced into half-moons"));
        assert_eq!(after[1].unit, "whole");
        // Index 2: clean
        assert_eq!(after[2].name, "salt");
        assert_eq!(after[2].unit, "tsp");
        // Index 3: pattern C
        assert_eq!(after[3].name, "salt");
        assert_eq!(after[3].unit, "teaspoon");
        assert_eq!(after[3].amount, single(0.25));
    }

    // ─── Loud failure on corrupt JSON ─────────────────────────────────

    #[tokio::test]
    async fn corrupt_ingredients_json_halts_migration() {
        let db = legacy_db().await;
        insert(&db, "r1", "{ this is not json").await;
        let result = Migration.up(&SchemaManager::new(&db)).await;
        let err = result.expect_err("migration should fail on corrupt JSON");
        let msg = err.to_string();
        assert!(msg.contains("r1"), "error should name the bad recipe id");
        assert!(
            msg.contains("ingredients JSON"),
            "error should explain the failure"
        );
    }

    #[tokio::test]
    async fn empty_ingredients_array_is_left_alone() {
        let db = legacy_db().await;
        insert(&db, "r1", "[]").await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert!(after.is_empty());
    }

    // ─── Pattern detector unit tests ──────────────────────────────────

    #[test]
    fn name_starts_with_failed_amount_recognizes_em_dash_range() {
        assert!(name_starts_with_failed_amount("12–15 fresh sage leaves"));
        assert!(name_starts_with_failed_amount("3—4 lbs ribs"));
        assert!(name_starts_with_failed_amount("2-3 balls pizza dough"));
    }

    #[test]
    fn name_starts_with_failed_amount_recognizes_unicode_fraction() {
        assert!(name_starts_with_failed_amount("¼ teaspoon salt"));
        assert!(name_starts_with_failed_amount("¾ cup flour"));
        assert!(name_starts_with_failed_amount("⅔ cup sugar"));
        assert!(name_starts_with_failed_amount("1½ cups milk"));
        assert!(name_starts_with_failed_amount("2¼ cups water"));
    }

    #[test]
    fn name_starts_with_failed_amount_rejects_normal_names() {
        assert!(!name_starts_with_failed_amount("garlic"));
        assert!(!name_starts_with_failed_amount("olive oil"));
        assert!(!name_starts_with_failed_amount("80/20 ground beef"));
        assert!(!name_starts_with_failed_amount("12 eggs")); // no range
        assert!(!name_starts_with_failed_amount(""));
    }

    #[test]
    fn is_failed_parse_signature() {
        assert!(is_failed_parse(&single(1.0), "to taste"));
        assert!(!is_failed_parse(&single(1.0), "whole"));
        assert!(!is_failed_parse(&single(2.0), "to taste"));
        assert!(!is_failed_parse(&range(1.0, 2.0), "to taste"));
    }
}
