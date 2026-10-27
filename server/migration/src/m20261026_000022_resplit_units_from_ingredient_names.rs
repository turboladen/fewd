use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement, TransactionTrait};
use serde_json::Value;

use crate::ingredient_amount::{is_known_unit, name_after_unit, split_leading_unit};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Re-splits recipe ingredients whose unit the markdown importer stored inside
/// the name. An entry with unit `whole` whose name starts with a two-word unit
/// ("fl oz milk"), or with a one-word unit that `legacy_is_known_unit` rejects
/// ("bunches kale", "oz. cheese"), gets that unit, as written, and the rest of
/// the name. One-letter units, box, kilo, pt and gal, dotted or not, are never
/// split off, because each can begin a name. An entry with any other
/// known unit drops a leading word "of" from its name.
///
/// It reads markdown imports and the recipes copied or adapted from them, and
/// runs in one transaction: unparsable JSON halts it with the recipe id and
/// changes nothing. Each change is printed to stderr.
//
// The unit helpers are shared with the import parser (migrations.md), so a
// database that runs this later reads units with the code it runs with.
// `legacy_is_known_unit` is frozen here and limits that drift to spellings
// the parser at commit 1209391~1 could not read.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // sea-orm-migration wraps only Postgres migrations in a transaction.
        let txn = manager.get_connection().begin().await?;
        let rows = txn
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                ELIGIBLE_RECIPES.to_string(),
            ))
            .await?;
        let mut report = Vec::new();
        for row in rows {
            let id: String = row.try_get("", "id")?;
            let original: String = row.try_get("", "ingredients")?;
            let mut list: Vec<Value> = serde_json::from_str(&original).map_err(|e| {
                DbErr::Custom(format!(
                    "recipe {id} has unprocessable ingredients JSON: {e}. \
                     Fix the row manually before re-running migrations."
                ))
            })?;
            let mut changes = Vec::new();
            for ingredient in &mut list {
                resplit(ingredient, &mut changes);
            }
            if changes.is_empty() {
                continue;
            }
            let rewritten = serde_json::to_string(&list).map_err(|e| {
                DbErr::Custom(format!(
                    "recipe {id}: rewritten ingredients did not serialize: {e}"
                ))
            })?;
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE recipes SET ingredients = ? WHERE id = ?".to_string(),
                [rewritten.into(), id.clone().into()],
            ))
            .await?;
            report.extend(
                changes
                    .into_iter()
                    .map(|change| format!("recipe {id}: {change}")),
            );
        }
        txn.commit().await?;
        // This rewrites text a person imported, so each change is reported
        // once it is committed. The migration crate has no tracing dependency,
        // hence eprintln.
        for line in &report {
            eprintln!("migration m20261026_000022: {line}");
        }
        if !report.is_empty() {
            eprintln!(
                "migration m20261026_000022: re-split {} ingredient(s)",
                report.len()
            );
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op. Joining the unit back into the name would restore a misparse.
        Ok(())
    }
}

// This selects markdown imports and every scaled copy or AI variant descended
// from one. A scaled copy and an MCP variant copy the parent's stored
// ingredients; the web adapt flow has the model rewrite them from the parent's,
// so its rows usually carry the parent's spellings too. UNION stops at a
// cycle, and a copy whose parent is gone is not reached.
const ELIGIBLE_RECIPES: &str = "
    WITH RECURSIVE eligible(id) AS (
        SELECT id FROM recipes WHERE source = 'markdown_import'
        UNION
        SELECT r.id FROM recipes r JOIN eligible e ON r.parent_recipe_id = e.id
        WHERE r.source IN ('ai_adapted', 'scaled')
    )
    SELECT r.id, r.ingredients FROM recipes r JOIN eligible USING (id) ORDER BY r.id";

// This repairs the entry and its alternative chain, and records each change.
// An entry whose name or unit is not a string is not judged, but its
// alternative still is.
fn resplit(ingredient: &mut Value, changes: &mut Vec<String>) {
    let Some(object) = ingredient.as_object_mut() else {
        return;
    };
    let fields = (object.get("name"), object.get("unit"));
    if let (Some(Value::String(name)), Some(Value::String(unit))) = fields {
        if let Some((new_unit, new_name)) = repair(name, unit) {
            changes.push(format!(
                "{name:?} ({unit:?}) -> {new_name:?} ({new_unit:?})"
            ));
            object.insert("unit".to_string(), Value::String(new_unit));
            object.insert("name".to_string(), Value::String(new_name));
        }
    }
    if let Some(alternative) = object.get_mut("or_alternative") {
        resplit(alternative, changes);
    }
}

// It returns the new unit and name, or None to leave the entry alone.
fn repair(name: &str, unit: &str) -> Option<(String, String)> {
    if unit == "whole" {
        let (written, rest) = split_leading_unit(name)?;
        // A stored name has lost the amount that tells "T butter" from
        // "T bone steak" or "T. Marzetti dressing". "box grater" and "Kilo
        // brand rice" may be typed by hand, and "Pt." and "Gal." abbreviate
        // name words ("Pt. Reyes blue cheese"), so those units are never split
        // off, dotted or not. The other kept spellings (dl, cl, tbl, and dotted
        // oz., lb., qt., tbsp., pkg.) abbreviate no name word.
        let core = written.strip_suffix('.').unwrap_or(&written).to_lowercase();
        let name_word = matches!(
            core.as_str(),
            "box" | "boxes" | "kilo" | "kilos" | "pt" | "pts" | "gal" | "gals"
        );
        if core.chars().count() <= 1 || name_word {
            return None;
        }
        // The 1209391~1 parser splits off every one-word unit that
        // `legacy_is_known_unit` accepts, so a whole row that starts with one
        // ("whole chicken", "strip steaks") did not come from that parser. That
        // parser splits no two-word unit, even one the predicate accepts
        // ("fl oz"), so a two-word unit skips this check.
        if !written.contains(' ') && legacy_is_known_unit(&written) {
            return None;
        }
        return Some((written, rest.to_string()));
    }
    let stripped = name_after_unit(unit, name);
    (is_known_unit(unit) && stripped != name).then(|| (unit.to_string(), stripped.to_string()))
}

// This is the one-token unit predicate of the markdown parser, copied
// verbatim from `ingredient_amount::is_known_unit` at commit 1209391~1 and
// frozen here.
fn legacy_is_known_unit(token: &str) -> bool {
    let stripped = token
        .strip_suffix(',')
        .unwrap_or(token)
        .trim()
        .to_lowercase();
    if stripped.is_empty() {
        return false;
    }

    // Multi-word special case (must come before plural stripping).
    if matches!(
        stripped.as_str(),
        "fl oz" | "fl. oz" | "fl. oz." | "fluid ounce" | "fluid ounces"
    ) {
        return true;
    }

    // Irregular plurals — keep the explicit map small. Add entries here
    // before the generic `s`-strip when the singular form differs by more
    // than one trailing letter.
    let singular = match stripped.as_str() {
        "leaves" => "leaf",
        other => {
            if other.ends_with('s') && other.len() > 2 {
                &other[..other.len() - 1]
            } else {
                other
            }
        }
    };

    matches!(
        singular,
        // Weight
        "g" | "gram" | "kg" | "kilogram" | "mg" | "milligram" | "oz" | "ounce" | "lb" | "pound"
        // Volume
        | "ml" | "milliliter" | "millilitre"
        | "l" | "liter" | "litre"
        | "cup" | "tbsp" | "tablespoon" | "tb" | "tbs"
        | "tsp" | "teaspoon" | "ts"
        | "pint" | "pt" | "quart" | "qt" | "gallon" | "gal"
        // Discrete cooking units
        | "whole" | "piece" | "clove" | "slice" | "can" | "sheet" | "head" | "bunch"
        | "sprig" | "bag" | "bottle" | "package" | "pkg" | "block" | "stalk" | "strip"
        | "pinch" | "dash" | "splash" | "drop" | "leaf" | "ball" | "sleeve" | "jar"
        | "container" | "stick" | "ear" | "shake"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database, DatabaseConnection};
    use serde_json::json;

    // The table has only the columns the migration reads and writes, under
    // their shipped names.
    const SCHEMA: &str = "CREATE TABLE recipes (id TEXT PRIMARY KEY, source TEXT NOT NULL,
        parent_recipe_id TEXT, ingredients TEXT NOT NULL);";

    async fn legacy_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(SCHEMA).await.unwrap();
        db
    }

    async fn run(db: &DatabaseConnection) -> Result<(), DbErr> {
        Migration.up(&SchemaManager::new(db)).await
    }

    async fn insert(
        db: &DatabaseConnection,
        id: &str,
        source: &str,
        parent: Option<&str>,
        ingredients: String,
    ) {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO recipes (id, source, parent_recipe_id, ingredients) VALUES (?, ?, ?, ?)",
            [
                id.into(),
                source.into(),
                parent.map(str::to_string).into(),
                ingredients.into(),
            ],
        ))
        .await
        .unwrap();
    }

    async fn raw(db: &DatabaseConnection, id: &str) -> String {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT ingredients FROM recipes WHERE id = ?",
                [id.into()],
            ))
            .await
            .unwrap()
            .unwrap();
        row.try_get("", "ingredients").unwrap()
    }

    async fn stored(db: &DatabaseConnection, id: &str) -> Value {
        serde_json::from_str(&raw(db, id).await).unwrap()
    }

    fn ing(name: &str, unit: &str) -> Value {
        json!({"name": name, "amount": {"type": "single", "value": 2.0}, "unit": unit, "notes": null})
    }

    fn pairs(list: &Value) -> Vec<(String, String)> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|i| {
                (
                    i["unit"].as_str().unwrap().to_string(),
                    i["name"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    fn expected(rows: &[(&str, &str)]) -> Vec<(String, String)> {
        rows.iter()
            .map(|&(unit, name)| (String::from(unit), String::from(name)))
            .collect()
    }

    // Seeds one markdown import holding every `(name, unit)`, runs the
    // migration, and returns the stored `(unit, name)` pairs.
    async fn after_run(rows: &[(&str, &str)]) -> Vec<(String, String)> {
        let db = legacy_db().await;
        let list: Vec<Value> = rows.iter().map(|(name, unit)| ing(name, unit)).collect();
        insert(&db, "r1", "markdown_import", None, json!(list).to_string()).await;
        run(&db).await.unwrap();
        pairs(&stored(&db, "r1").await)
    }

    #[tokio::test]
    async fn splits_es_plural_units() {
        let rows = [
            ("bunches kale", "whole"),
            ("pinches salt", "whole"),
            ("dashes bitters", "whole"),
            ("splashes milk", "whole"),
        ];
        assert_eq!(
            after_run(&rows).await,
            expected(&[
                ("bunches", "kale"),
                ("pinches", "salt"),
                ("dashes", "bitters"),
                ("splashes", "milk"),
            ])
        );
    }

    #[tokio::test]
    async fn splits_dotted_short_and_two_word_units() {
        let rows = [
            ("oz. cheese", "whole"),
            ("Tbsp. butter", "whole"),
            ("dl milk", "whole"),
            ("tbl butter", "whole"),
            ("cl gin", "whole"),
            ("fl oz milk", "whole"),
            ("fluid oz milk", "whole"),
        ];
        assert_eq!(
            after_run(&rows).await,
            expected(&[
                ("oz.", "cheese"),
                ("Tbsp.", "butter"),
                ("dl", "milk"),
                ("tbl", "butter"),
                ("cl", "gin"),
                ("fl oz", "milk"),
                ("fluid oz", "milk"),
            ])
        );
    }

    #[tokio::test]
    async fn keeps_amount_prep_and_notes_of_a_split_row() {
        let db = legacy_db().await;
        let row = json!({"name": "pinches salt", "prep": "crushed", "unit": "whole",
            "amount": {"type": "range", "min": 1.0, "max": 2.0}, "notes": "flaky"});
        insert(&db, "r1", "markdown_import", None, json!([row]).to_string()).await;
        run(&db).await.unwrap();
        assert_eq!(
            stored(&db, "r1").await,
            json!([{"name": "salt", "prep": "crushed", "unit": "pinches",
                "amount": {"type": "range", "min": 1.0, "max": 2.0}, "notes": "flaky"}])
        );
    }

    #[tokio::test]
    async fn drops_of_when_splitting_and_after_a_real_unit() {
        let rows = [
            ("pinches of salt", "whole"),
            ("fl oz of milk", "whole"),
            ("of flour", "cups"),
            ("Of beans", "can"),
            ("of the eggs", "whole"),
            ("of the eggs", ""),
            ("of salt", "to taste"),
        ];
        assert_eq!(
            after_run(&rows).await,
            expected(&[
                ("pinches", "salt"),
                ("fl oz", "milk"),
                ("cups", "flour"),
                ("can", "beans"),
                ("whole", "of the eggs"),
                ("", "of the eggs"),
                ("to taste", "of salt"),
            ])
        );
    }

    #[tokio::test]
    async fn leaves_hand_written_and_ambiguous_rows_alone() {
        let rows = [
            ("T bone steak", "whole"),
            ("C clamp", "whole"),
            ("t bones", "whole"),
            ("g force bars", "whole"),
            ("L brand pasta", "whole"),
            ("T. butter", "whole"),
            ("c. sugar", "whole"),
            ("T-bone steak", "whole"),
            ("box grater", "whole"),
            ("boxes cereal", "whole"),
            ("Kilo brand rice", "whole"),
            ("kilos potatoes", "whole"),
            ("Pt. Reyes blue cheese", "whole"),
            ("Gal. pal cookies", "whole"),
            ("pts. Reyes toma", "whole"),
            ("whole chicken", "whole"),
            ("whole wheat tortillas", "whole"),
            ("strip steaks", "whole"),
            ("head cheese", "whole"),
            ("glasses wine", "whole"),
            ("cups. flour", "whole"),
            ("bunches", "whole"),
            ("fresh sage leaves", "whole"),
            ("zucchini", "whole"),
            ("bunches kale", "cups"),
            ("bunches kale", "Whole"),
        ];
        let untouched: Vec<(&str, &str)> = rows.iter().map(|(name, unit)| (*unit, *name)).collect();
        assert_eq!(after_run(&rows).await, expected(&untouched));
    }

    // The importer moves anything after a comma into prep, so a stored name
    // never carries one. "2 bunches, kale" is stored as name "bunches" and
    // prep "kale", which has no name left to split.
    #[tokio::test]
    async fn a_unit_with_its_name_in_prep_is_left_alone() {
        let db = legacy_db().await;
        let row = json!({"name": "bunches", "prep": "kale", "unit": "whole",
            "amount": {"type": "single", "value": 2.0}});
        let original = json!([row]).to_string();
        insert(&db, "r1", "markdown_import", None, original.clone()).await;
        run(&db).await.unwrap();
        assert_eq!(raw(&db, "r1").await, original);
    }

    #[tokio::test]
    async fn walks_alternative_chains_past_entries_it_cannot_judge() {
        let db = legacy_db().await;
        let mut innermost = ing("bunches kale", "whole");
        innermost["or_alternative"] = Value::Null;
        let mut middle =
            json!({"name": "spinach", "unit": null, "amount": {"type": "single", "value": 1.0}});
        middle["or_alternative"] = innermost;
        let mut primary = ing("chard", "whole");
        primary["or_alternative"] = middle;
        let numeric = json!({"name": "bunches kale", "unit": 3});
        let list = json!([primary, numeric, "not an object", 7]);
        insert(&db, "r1", "markdown_import", None, list.to_string()).await;

        run(&db).await.unwrap();

        let after = stored(&db, "r1").await;
        assert_eq!(after[0]["name"], "chard");
        assert_eq!(after[0]["or_alternative"]["unit"], Value::Null);
        let split = &after[0]["or_alternative"]["or_alternative"];
        assert_eq!(
            (&split["unit"], &split["name"]),
            (&json!("bunches"), &json!("kale"))
        );
        assert_eq!(split["or_alternative"], Value::Null);
        assert_eq!(after[1], json!({"name": "bunches kale", "unit": 3}));
        assert_eq!(after[2], "not an object");
        assert_eq!(after[3], 7);
    }

    #[tokio::test]
    async fn includes_copies_and_variants_of_markdown_imports() {
        let db = legacy_db().await;
        let list = || json!([ing("bunches kale", "whole")]).to_string();
        insert(&db, "md", "markdown_import", None, list()).await;
        insert(&db, "variant", "ai_adapted", Some("md"), list()).await;
        insert(&db, "variant2", "ai_adapted", Some("variant"), list()).await;
        insert(&db, "scaled", "scaled", Some("md"), list()).await;
        insert(&db, "scaled-variant", "ai_adapted", Some("scaled"), list()).await;
        insert(&db, "manual", "manual", None, list()).await;
        insert(&db, "manual-variant", "ai_adapted", Some("manual"), list()).await;
        insert(&db, "orphan", "ai_adapted", Some("gone"), list()).await;
        insert(&db, "url", "url_import", None, list()).await;
        insert(&db, "url-of-md", "url_import", Some("md"), list()).await;

        run(&db).await.unwrap();

        for id in ["md", "variant", "variant2", "scaled", "scaled-variant"] {
            assert_eq!(
                pairs(&stored(&db, id).await),
                expected(&[("bunches", "kale")]),
                "{id}"
            );
        }
        for id in ["manual", "manual-variant", "orphan", "url", "url-of-md"] {
            assert_eq!(raw(&db, id).await, list(), "{id}");
        }
    }

    #[tokio::test]
    async fn untouched_recipes_stay_byte_for_byte_and_a_rerun_changes_nothing() {
        let db = legacy_db().await;
        // This key order and spacing are ones serde_json would not reproduce.
        let clean =
            r#"[{"unit": "bunches", "name": "kale", "amount": {"type": "single", "value": 2}}]"#;
        insert(&db, "clean", "markdown_import", None, clean.to_string()).await;
        let messy = json!([
            ing("bunches kale", "whole"),
            ing("pinches of salt", "whole"),
            ing("of flour", "cups"),
        ]);
        insert(&db, "messy", "markdown_import", None, messy.to_string()).await;

        run(&db).await.unwrap();
        let after_first = raw(&db, "messy").await;
        run(&db).await.unwrap();

        assert_eq!(raw(&db, "clean").await, clean);
        assert_eq!(raw(&db, "messy").await, after_first);
        assert_eq!(
            pairs(&stored(&db, "messy").await),
            expected(&[("bunches", "kale"), ("pinches", "salt"), ("cups", "flour")])
        );
    }

    #[tokio::test]
    async fn unknown_keys_survive_a_rewrite() {
        let db = legacy_db().await;
        let mut alternative = ing("dashes bitters", "whole");
        alternative["brand"] = json!("Angostura");
        let mut primary = ing("splashes soda", "whole");
        primary["future_field"] = json!({"nested": [1, 2]});
        primary["or_alternative"] = alternative;
        insert(
            &db,
            "r1",
            "markdown_import",
            None,
            json!([primary]).to_string(),
        )
        .await;

        run(&db).await.unwrap();

        let after = stored(&db, "r1").await;
        assert_eq!(after[0]["future_field"], json!({"nested": [1, 2]}));
        assert_eq!(after[0]["name"], "soda");
        assert_eq!(after[0]["or_alternative"]["brand"], "Angostura");
        assert_eq!(after[0]["or_alternative"]["name"], "bitters");
    }

    #[tokio::test]
    async fn unprocessable_json_halts_with_the_recipe_id_and_changes_nothing() {
        for (id, ingredients) in [("broken", "[{"), ("not-a-list", r#"{"name": "kale"}"#)] {
            let db = legacy_db().await;
            // "a-fixable" sorts before the bad row, so it is rewritten first.
            let fixable = json!([ing("bunches kale", "whole")]).to_string();
            insert(&db, "a-fixable", "markdown_import", None, fixable.clone()).await;
            insert(&db, id, "markdown_import", None, ingredients.to_string()).await;
            match run(&db).await {
                Err(DbErr::Custom(message)) => assert!(message.contains(id), "{message}"),
                other => panic!("expected a halt for {id}, got {other:?}"),
            }
            assert_eq!(raw(&db, "a-fixable").await, fixable, "{id}");
        }
    }
}
