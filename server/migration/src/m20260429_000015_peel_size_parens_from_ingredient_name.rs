use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::paren_notes::peel_size_paren;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Walks every recipe and repairs ingredient rows whose `name` still
/// carries a leading or mid-string size-info parenthetical that the
/// pre-fewd-i47 parser left embedded.
///
/// Pattern: ingredient lines like `2 cans (28 oz each) crushed San Marzano
/// tomatoes` were misparsed by the runtime into `name="(28 oz each) crushed
/// San Marzano tomatoes", unit="cans"`. The fix in `recipe_parser` peels
/// `(28 oz each)` into `notes` before tokenization, but existing rows on
/// the dietpi DB are still in the corrupted shape. This migration runs the
/// same `peel_size_paren` helper over every stored `name`; rows where the
/// helper produces notes get rewritten in place, everything else passes
/// through.
///
/// Notes precedence: when the row already has a `notes` field, the peeled
/// content is appended with `"; "` so an operator can still see the
/// pre-existing context. In practice the bug produced rows with
/// `notes = None`, so this is purely defensive.
///
/// Scoped to `recipes` only — drink_recipes / meals / templates carry no
/// ingredient data per the dietpi audit. Idempotent: a peeled name no
/// longer contains parens, so a second pass is a no-op.
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
        // No-op. Reverting a repaired row would require re-corrupting it
        // by re-embedding the parenthetical into `name`. There's no value
        // in fabricating the misparsed shape.
        Ok(())
    }
}

/// Frozen-in-time copy of the ingredient shape (per CLAUDE.md, migrations
/// own their structs; do not share with m13/m14 even though the shapes are
/// currently identical).
#[derive(Debug, Deserialize, Serialize)]
struct Ingredient {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prep: Option<String>,
    amount: Value,
    #[serde(default)]
    unit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    /// Generic unknown-fields passthrough. Captures anything the runtime
    /// IngredientDto grows AFTER this migration shipped (fewd-4nb's
    /// `or_alternative` is the first such field). Without this, every new
    /// optional DTO field would be silently erased on rewrite — see
    /// fewd-2y6.2. The migration body never reads `extra`; serde just
    /// round-trips it through deserialize/serialize.
    #[serde(flatten)]
    extra: Map<String, Value>,
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

fn reparse_ingredient(ing: &mut Ingredient) -> bool {
    let (cleaned, peeled_notes) = peel_size_paren(&ing.name);
    let Some(peeled) = peeled_notes else {
        return false;
    };
    ing.name = cleaned;
    ing.notes = match ing.notes.take() {
        Some(existing) if !existing.trim().is_empty() => Some(format!("{existing}; {peeled}")),
        _ => Some(peeled),
    };
    true
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

    fn array_one(ing: Value) -> String {
        serde_json::to_string(&[ing]).unwrap()
    }

    #[tokio::test]
    async fn peels_leading_size_parens_from_name() {
        // The fewd-i47 hero shape: pre-fix runtime stored the parenthetical
        // size info as a leading prefix on `name`. The migration runs the
        // same peel_size_paren helper the runtime now uses, producing the
        // shape the post-fix parser produces fresh.
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "(28 oz each) crushed San Marzano tomatoes",
                None,
                single(2.0),
                "cans",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "crushed San Marzano tomatoes");
        assert_eq!(after[0].unit, "cans");
        assert_eq!(after[0].amount, single(2.0));
        assert_eq!(after[0].notes.as_deref(), Some("28 oz each"));
        assert_eq!(after[0].prep, None);
    }

    #[tokio::test]
    async fn peels_mid_string_size_parens_from_name() {
        // Hypothetical: a row stored with the parens mid-string instead of
        // at the start. peel_size_paren handles both shapes.
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "chicken breasts (about 1 lb total) boneless",
                None,
                single(3.0),
                "whole",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "chicken breasts boneless");
        assert_eq!(after[0].notes.as_deref(), Some("about 1 lb total"));
    }

    #[tokio::test]
    async fn passes_through_alternative_parens() {
        // The fewd-xez case must NOT be peeled: the parens carry an
        // alternative noun, no unit token inside, and the row is otherwise
        // fine. Migration leaves it alone.
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "Asian pear (or Fuji apple)",
                Some("grated"),
                single(1.0),
                "whole",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "Asian pear (or Fuji apple)");
        assert_eq!(after[0].prep.as_deref(), Some("grated"));
    }

    #[tokio::test]
    async fn passes_through_clean_rows() {
        // No parens in name → no rewrite.
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
    async fn passes_through_trailing_parens() {
        // Notes-style trailing parens in `name` are out of scope for this
        // migration (the runtime's `extract_notes` handles them; if a row
        // ever ended up with trailing parens still in name, that's a
        // different bug). peel_size_paren requires a word-char suffix
        // after `)`, so trailing parens fall through.
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "orange juice (fresh is best)",
                None,
                single(1.0),
                "cup",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "orange juice (fresh is best)");
        assert_eq!(after[0].notes, None);
    }

    #[tokio::test]
    async fn merges_with_existing_notes() {
        // Defensive: a row with both a parens-prefixed name AND a
        // pre-existing notes field. We must not silently drop the existing
        // notes — append peeled content with "; ".
        let db = legacy_db().await;
        let mut row = ing_json("(200 g) chopped onions", None, single(1.0), "whole");
        row["notes"] = json!("from the garden");
        insert(&db, "r1", &array_one(row)).await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "chopped onions");
        assert_eq!(after[0].notes.as_deref(), Some("from the garden; 200 g"));
    }

    #[tokio::test]
    async fn idempotent_on_rerun() {
        let db = legacy_db().await;
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "(28 oz each) crushed San Marzano tomatoes",
                None,
                single(2.0),
                "cans",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after_first = ingredients(&db, "r1").await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after_second = ingredients(&db, "r1").await;
        assert_eq!(after_first[0].name, after_second[0].name);
        assert_eq!(after_first[0].notes, after_second[0].notes);
        assert_eq!(after_first[0].unit, after_second[0].unit);
    }

    #[tokio::test]
    async fn mixed_recipe_only_updates_changed_ingredients() {
        let db = legacy_db().await;
        let mixed = serde_json::to_string(&[
            ing_json("garlic", Some("minced"), single(3.0), "cloves"), // clean
            ing_json(
                "(28 oz each) crushed San Marzano tomatoes",
                None,
                single(2.0),
                "cans",
            ), // pattern paren-size
            ing_json("salt", None, single(1.0), "tsp"),                // clean
        ])
        .unwrap();
        insert(&db, "r1", &mixed).await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "garlic");
        assert_eq!(after[1].name, "crushed San Marzano tomatoes");
        assert_eq!(after[1].notes.as_deref(), Some("28 oz each"));
        assert_eq!(after[2].name, "salt");
    }

    #[tokio::test]
    async fn corrupt_ingredients_json_halts_migration() {
        let db = legacy_db().await;
        insert(&db, "r1", "{ this is not json").await;
        let result = Migration.up(&SchemaManager::new(&db)).await;
        let err = result.expect_err("migration should fail on corrupt JSON");
        let msg = format!("{err}");
        assert!(
            msg.contains("recipe r1"),
            "error should name the recipe id: {msg}"
        );
    }

    /// Calibration: feed the migration the exact post-bug shape produced by
    /// the runtime parser pre-fix, and confirm the post-migration row matches
    /// the shape the runtime parser now produces fresh from the same markdown.
    /// Closes the loop between the forward-fix and the backfill.
    #[tokio::test]
    async fn live_data_calibration_matches_runtime_parser() {
        let db = legacy_db().await;
        // Pre-fix runtime would have stored the Bolognese line as:
        insert(
            &db,
            "r1",
            &array_one(ing_json(
                "(28 oz each) crushed San Marzano tomatoes",
                None,
                single(2.0),
                "cans",
            )),
        )
        .await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let after = ingredients(&db, "r1").await;

        // Post-fix runtime parsing the same markdown produces:
        //   name="crushed San Marzano tomatoes", unit="cans", amount=2,
        //   notes="28 oz each", prep=None
        // (covered by recipe_parser::tests::test_ingredient_with_mid_string_size_parens)
        assert_eq!(after[0].name, "crushed San Marzano tomatoes");
        assert_eq!(after[0].unit, "cans");
        assert_eq!(after[0].amount, single(2.0));
        assert_eq!(after[0].notes.as_deref(), Some("28 oz each"));
        assert_eq!(after[0].prep, None);
    }

    /// Regression: fewd-2y6.2. The frozen struct uses
    /// `#[serde(flatten)] extra: Map<String, Value>` to round-trip every
    /// unknown field generically. fewd-4nb's `or_alternative` was the
    /// first such field; the test fixture also includes a `future_field`
    /// to pin the generic contract for the next addition.
    #[tokio::test]
    async fn preserves_unknown_fields_through_rewrite() {
        let db = legacy_db().await;
        // Mid-string size parens force a rewrite path; or_alternative +
        // a fully-unknown field both ride along and must survive.
        let json = r#"[{
            "name":"(28 oz each) crushed tomatoes",
            "amount":{"type":"single","value":2.0},
            "unit":"cans",
            "notes":null,
            "or_alternative":{
                "name":"fresh Roma tomatoes",
                "amount":{"type":"single","value":3.0},
                "unit":"lb",
                "notes":null
            },
            "future_field":"hypothetical post-m15 DTO field"
        }]"#;
        insert(&db, "r1", json).await;

        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        // Read raw column so we assert on stored bytes, not a re-parse.
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT ingredients FROM recipes WHERE id = ?".to_string(),
                ["r1".into()],
            ))
            .await
            .unwrap()
            .unwrap();
        let raw: String = row.try_get("", "ingredients").unwrap();
        let parsed: Value = serde_json::from_str(&raw).unwrap();

        // Verify the size-paren peel actually executed — without these
        // assertions the test would still pass if the migration silently
        // no-op'd, since the input already contains the unknown fields.
        assert_eq!(
            parsed[0]["name"], "crushed tomatoes",
            "size-paren peel should have stripped the leading paren from name"
        );
        assert_eq!(
            parsed[0]["notes"], "28 oz each",
            "size-paren peel should have moved the size info into notes"
        );

        // Both the named-but-not-declared field and a fully unknown field
        // round-tripped cleanly via #[serde(flatten)] extra.
        let alt = &parsed[0]["or_alternative"];
        assert!(alt.is_object(), "or_alternative must round-trip; got {raw}");
        assert_eq!(alt["name"], "fresh Roma tomatoes");
        assert_eq!(
            parsed[0]["future_field"], "hypothetical post-m15 DTO field",
            "arbitrary unknown fields must round-trip; got {raw}"
        );
    }
}
